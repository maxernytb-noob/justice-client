// Drop-in replacement for <select>: same value / onChange(e.target.value) / <option> children, but a
// themed popup (the webview's native one ignores the dark theme and can come out white).
import { Children, isValidElement, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../icons";
import { tr } from "../i18n";

type Opt = { value: string; label: ReactNode; text: string; disabled?: boolean; group?: string };

function textOf(n: ReactNode): string {
  if (n == null || typeof n === "boolean") return "";
  if (typeof n === "string" || typeof n === "number") return String(n);
  if (Array.isArray(n)) return n.map(textOf).join("");
  if (isValidElement<{ children?: ReactNode }>(n)) return textOf(n.props.children);
  return "";
}

function collect(children: ReactNode, group?: string, out: Opt[] = []): Opt[] {
  Children.forEach(children, (c) => {
    if (!isValidElement<{ value?: string | number; children?: ReactNode; disabled?: boolean; label?: string }>(c)) return;
    if (c.type === "option") {
      const label = c.props.children;
      out.push({ value: String(c.props.value ?? textOf(label)), label, text: textOf(label), disabled: c.props.disabled, group });
    } else if (c.type === "optgroup") {
      collect(c.props.children, c.props.label, out);
    } else {
      collect(c.props.children, group, out);
    }
  });
  return out;
}

export default function Select({
  value,
  onChange,
  disabled,
  className = "select",
  style,
  children,
  title,
}: {
  value?: string | number;
  onChange?: (e: { target: { value: string } }) => void;
  disabled?: boolean;
  className?: string;
  style?: React.CSSProperties;
  children?: ReactNode;
  title?: string;
}) {
  const opts = useMemo(() => collect(children), [children]);
  const cur = opts.find((o) => o.value === String(value ?? "")) ?? (value === undefined ? opts[0] : undefined);
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [hi, setHi] = useState(0);
  const [pos, setPos] = useState<{ left: number; top: number; width: number; up: boolean; maxH: number } | null>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const searchable = opts.length > 10;
  const shown = useMemo(() => (q ? opts.filter((o) => o.text.toLowerCase().includes(q.toLowerCase())) : opts), [opts, q]);

  const place = () => {
    const r = btn.current?.getBoundingClientRect();
    if (!r) return;
    const below = innerHeight - r.bottom - 12, above = r.top - 12;
    const up = below < 240 && above > below;
    setPos({ left: r.left, top: up ? r.top - 6 : r.bottom + 6, width: Math.max(r.width, 180), up, maxH: Math.min(340, Math.max(160, up ? above : below)) });
  };
  const show = () => {
    if (disabled) return;
    place();
    setQ("");
    setHi(Math.max(0, opts.findIndex((o) => o.value === String(value ?? ""))));
    setOpen(true);
  };
  const pick = (o: Opt) => {
    if (o.disabled) return;
    setOpen(false);
    if (o.value !== String(value ?? "")) onChange?.({ target: { value: o.value } });
    btn.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const close = (e: Event) => {
      const t = e.target as Node;
      if (list.current?.parentElement?.contains(t) || btn.current?.contains(t)) return;
      setOpen(false);
    };
    const reposition = (e: Event) => {
      if (list.current?.contains(e.target as Node)) return;
      setOpen(false);
    };
    addEventListener("pointerdown", close, true);
    addEventListener("scroll", reposition, true);
    addEventListener("resize", reposition);
    return () => {
      removeEventListener("pointerdown", close, true);
      removeEventListener("scroll", reposition, true);
      removeEventListener("resize", reposition);
    };
  }, [open]);
  // Keep the highlighted row in view.
  useLayoutEffect(() => {
    if (!open) return;
    list.current?.querySelector<HTMLElement>(`[data-i="${hi}"]`)?.scrollIntoView({ block: "nearest" });
  }, [hi, open, shown.length]);
  useEffect(() => setHi(0), [q]);

  const key = (e: React.KeyboardEvent) => {
    if (!open) {
      if (["Enter", " ", "ArrowDown", "ArrowUp"].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setHi((h) => Math.min(shown.length - 1, h + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHi((h) => Math.max(0, h - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown[hi]) pick(shown[hi]);
    } else if (!searchable && e.key.length === 1) {
      // Type-ahead without a search box.
      const i = shown.findIndex((o) => o.text.toLowerCase().startsWith(e.key.toLowerCase()));
      if (i >= 0) setHi(i);
    }
  };

  return (
    <>
      <button
        ref={btn}
        type="button"
        className={`jselect ${className} ${open ? "open" : ""}`}
        style={style}
        disabled={disabled}
        title={title}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={key}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span className="jselect-val ellipsis">{cur ? cur.label : <span className="muted">—</span>}</span>
        <Icon name="down" size={14} />
      </button>
      {open &&
        pos &&
        createPortal(
          <div
            className={`jselect-pop ${pos.up ? "up" : ""}`}
            style={{ left: pos.left, width: pos.width, [pos.up ? "bottom" : "top"]: pos.up ? innerHeight - pos.top : pos.top, maxHeight: pos.maxH } as React.CSSProperties}
            onKeyDown={key}
          >
            {searchable && (
              <div className="jselect-search">
                <Icon name="search" size={14} />
                <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder={tr("Hledat…", "Search…")} />
              </div>
            )}
            <div className="jselect-list" ref={list} role="listbox">
              {shown.length === 0 && <div className="jselect-empty">{tr("Nic nenalezeno", "Nothing found")}</div>}
              {shown.map((o, i) => (
                <div key={`${o.value}-${i}`}>
                  {o.group && (i === 0 || shown[i - 1].group !== o.group) && <div className="jselect-group">{o.group}</div>}
                  <div
                    data-i={i}
                    role="option"
                    aria-selected={o.value === String(value ?? "")}
                    className={`jselect-opt ${o.value === String(value ?? "") ? "sel" : ""} ${i === hi ? "hi" : ""} ${o.disabled ? "dis" : ""}`}
                    onPointerEnter={() => setHi(i)}
                    onClick={() => pick(o)}
                  >
                    <span className="ellipsis">{o.label}</span>
                    {o.value === String(value ?? "") && <Icon name="check" size={14} />}
                  </div>
                </div>
              ))}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
