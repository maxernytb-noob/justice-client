"""Builds web/<doc>.html from the bilingual markdown in this folder.

Usage: python build.py   (no dependencies; supports the markdown subset the documents use)
"""
import html
import pathlib
import re

ROOT = pathlib.Path(__file__).parent
DOCS = [
    ("terms", "Podmínky použití", "Terms of Use"),
    ("privacy", "Ochrana soukromí", "Privacy"),
    ("content-policy", "Pravidla obsahu", "Content Policy"),
]


def inline(text):
    text = html.escape(text, quote=False)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', text)
    text = re.sub(r"\[DOPLNIT:[^\]]*\]", lambda m: f'<mark class="todo">{m.group(0)}</mark>', text)
    return text


def slug(text, used):
    s = re.sub(r"[^\w]+", "-", text.lower()).strip("-") or "s"
    base, n = s, 2
    while s in used:
        s, n = f"{base}-{n}", n + 1
    used.add(s)
    return s


def render(md, prefix):
    """Markdown subset → (html, toc entries for h2)."""
    out, toc, used = [], [], set()
    lines = md.strip("\n").split("\n")
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        m = re.match(r"(#{1,3}) (.*)", line)
        if m:
            level, text = len(m.group(1)), m.group(2)
            if level == 1:
                out.append(f"<h1>{inline(text)}</h1>")
            else:
                sid = f"{prefix}-{slug(text, used)}"
                if level == 2:
                    toc.append((sid, text))
                out.append(f'<h{level} id="{sid}">{inline(text)}</h{level}>')
            i += 1
            continue
        if line.startswith(">"):
            buf = []
            while i < len(lines) and lines[i].startswith(">"):
                buf.append(lines[i].lstrip("> "))
                i += 1
            out.append(f'<aside class="note">{inline(" ".join(buf))}</aside>')
            continue
        if line.startswith("|"):
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            head, body = rows[0], [r for r in rows[1:] if not re.fullmatch(r"[-: ]*", "".join(r))]
            t = "<thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in head) + "</tr></thead><tbody>"
            for r in body:
                t += "<tr>" + "".join(f'<td data-h="{html.escape(h)}">{inline(c)}</td>' for h, c in zip(head, r)) + "</tr>"
            out.append(f'<div class="table"><table>{t}</tbody></table></div>')
            continue
        lm = re.match(r"(- |\d+\. )", line)
        if lm:
            tag = "ul" if line.startswith("- ") else "ol"
            items = []
            while i < len(lines) and re.match(r"(- |\d+\. )", lines[i]):
                items.append(re.sub(r"^(- |\d+\. )", "", lines[i]))
                i += 1
            out.append(f"<{tag}>" + "".join(f"<li>{inline(x)}</li>" for x in items) + f"</{tag}>")
            continue
        buf = []
        while i < len(lines) and lines[i].strip() and not re.match(r"(#|>|\||- |\d+\. )", lines[i]):
            buf.append(lines[i])
            i += 1
        para = " ".join(buf)
        cls = ' class="meta"' if para.startswith("**Verze:**") or para.startswith("**Version:**") else ""
        out.append(f"<p{cls}>{inline(para)}</p>")
    return "\n".join(out), toc


def split_langs(src):
    src = re.sub(r"\A---\n.*?\n---\n", "", src, flags=re.S)
    parts = re.split(r"<!-- lang:(\w+) -->", src)
    return {parts[k]: parts[k + 1] for k in range(1, len(parts), 2)}


def page(doc, cs_name, en_name):
    langs = split_langs((ROOT / f"{doc}.md").read_text(encoding="utf-8"))
    arts, navs = [], []
    for lang in ("cs", "en"):
        body, toc = render(langs[lang], lang)
        arts.append(f'<article lang="{lang}" data-lang="{lang}">{body}</article>')
        links = "".join(f'<a href="#{sid}">{inline(t)}</a>' for sid, t in toc)
        label = "Obsah" if lang == "cs" else "Contents"
        navs.append(f'<nav class="toc" data-lang="{lang}"><b>{label}</b>{links}</nav>')
    tabs = []
    for d, cs, en in DOCS:
        cur = ' aria-current="page"' if d == doc else ""
        tabs.append(f'<a href="{d}.html"{cur}><span data-lang="cs">{cs}</span><span data-lang="en">{en}</span></a>')
    return TEMPLATE.format(
        title_cs=cs_name, title_en=en_name, tabs="".join(tabs), toc="".join(navs), articles="".join(arts)
    )


TEMPLATE = """<!doctype html>
<html lang="cs">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title_cs} · Justice Client</title>
<link rel="icon" href="icon.png">
<style>
:root{{--bg:#060607;--bg2:#0b0b0d;--panel:#111114;--panel2:#16161a;--line:#222228;--line2:#2d2d35;--text:#f1f0f3;--text2:#a6a4ad;--text3:#6d6b75;--red:#e11d38;--red2:#c8102e;--reddim:#2a0a10}}
*{{box-sizing:border-box}}
html{{scroll-behavior:smooth;scroll-padding-top:84px}}
body{{margin:0;background:var(--bg);color:var(--text);font:16px/1.7 "Segoe UI",system-ui,-apple-system,sans-serif;-webkit-font-smoothing:antialiased}}
body::before{{content:"";position:fixed;inset:0 0 auto;height:420px;background:radial-gradient(600px 300px at 20% -10%,rgba(225,29,56,.16),transparent 70%);pointer-events:none}}
a{{color:#ff5a72;text-decoration:none}}a:hover{{text-decoration:underline}}
header{{position:sticky;top:0;z-index:5;background:rgba(6,6,7,.82);backdrop-filter:blur(12px);border-bottom:1px solid var(--line)}}
.bar{{max-width:1180px;margin:0 auto;padding:12px 16px;display:flex;align-items:center;gap:18px;flex-wrap:wrap}}
.brand{{display:flex;align-items:center;gap:10px;color:var(--text);font:800 17px/1 ui-monospace,Consolas,monospace;letter-spacing:.08em}}
.brand i{{width:28px;height:28px;border-radius:8px;background:linear-gradient(135deg,var(--red),var(--red2));display:grid;place-items:center;font-style:normal;font-size:15px;box-shadow:0 0 18px rgba(225,29,56,.35)}}
.brand:hover{{text-decoration:none}}
.tabs{{display:flex;gap:4px;flex-wrap:wrap;flex:1}}
.tabs a{{color:var(--text2);padding:7px 12px;border-radius:999px;font-size:14px}}
.tabs a:hover{{color:var(--text);background:var(--panel2);text-decoration:none}}
.tabs a[aria-current]{{color:var(--text);background:var(--reddim);box-shadow:inset 0 0 0 1px rgba(225,29,56,.45)}}
.lang{{display:flex;background:var(--panel);border:1px solid var(--line2);border-radius:999px;padding:3px}}
.lang button{{all:unset;cursor:pointer;padding:4px 12px;border-radius:999px;font:600 12.5px/1.4 ui-monospace,Consolas,monospace;color:var(--text3)}}
.lang button[aria-pressed=true]{{background:var(--red);color:#fff}}
.wrap{{max-width:1180px;margin:0 auto;padding:40px 16px 80px;display:grid;grid-template-columns:240px minmax(0,1fr);gap:48px;position:relative}}
.toc{{position:sticky;top:84px;align-self:start;max-height:calc(100vh - 110px);overflow:auto;font-size:13.5px;line-height:1.45;display:flex;flex-direction:column;gap:2px}}
.toc b{{font:600 11px/1 ui-monospace,Consolas,monospace;letter-spacing:.12em;text-transform:uppercase;color:var(--text3);margin:4px 0 10px}}
.toc a{{color:var(--text2);padding:5px 10px;border-left:2px solid var(--line)}}
.toc a:hover{{color:var(--text);border-color:var(--red);text-decoration:none}}
article{{max-width:780px;min-width:0}}
h1{{font-size:clamp(30px,5vw,44px);line-height:1.15;margin:0 0 18px;letter-spacing:-.02em}}
h2{{font-size:22px;margin:44px 0 12px;padding-top:20px;border-top:1px solid var(--line)}}
h3{{font-size:17px;margin:28px 0 8px;color:var(--text)}}
p,li{{color:#d6d4db}}
p{{margin:0 0 14px}}
ul,ol{{margin:0 0 16px;padding-left:22px}}li{{margin:4px 0}}li::marker{{color:var(--red)}}
strong{{color:var(--text)}}
code{{font:13px ui-monospace,Consolas,monospace;background:var(--panel2);border:1px solid var(--line);border-radius:6px;padding:1px 6px}}
.meta{{color:var(--text3);font-size:14px}}.meta strong{{color:var(--text2)}}
.note{{background:linear-gradient(180deg,rgba(225,29,56,.10),rgba(225,29,56,.04));border:1px solid rgba(225,29,56,.35);border-radius:14px;padding:14px 18px;margin:0 0 18px;font-size:14.5px;color:#e9c9cf}}
.note strong{{color:#fff}}
mark.todo{{background:rgba(250,204,21,.14);color:#fde68a;border:1px dashed rgba(250,204,21,.5);border-radius:6px;padding:0 5px;font-weight:600}}
.table{{overflow-x:auto;margin:6px 0 20px;border:1px solid var(--line);border-radius:14px;background:var(--bg2)}}
table{{border-collapse:collapse;width:100%;font-size:14px;line-height:1.5}}
th{{text-align:left;font:600 11.5px/1.3 ui-monospace,Consolas,monospace;letter-spacing:.06em;text-transform:uppercase;color:var(--text3);background:var(--panel);padding:11px 14px;border-bottom:1px solid var(--line)}}
td{{padding:11px 14px;border-top:1px solid var(--line);vertical-align:top;color:#d6d4db}}
tr:first-child td{{border-top:0}}
footer{{border-top:1px solid var(--line);color:var(--text3);font-size:13px;text-align:center;padding:28px 16px}}
html[data-lang=cs] [data-lang=en],html[data-lang=en] [data-lang=cs]{{display:none!important}}
@media (max-width:900px){{.wrap{{grid-template-columns:minmax(0,1fr);gap:0;padding-top:28px}}.toc{{display:none!important}}}}
@media (max-width:640px){{
 .tabs{{order:3;flex-basis:100%}}
 .table{{border:0;background:none}}
 table,tbody,tr,td{{display:block}}thead{{display:none}}
 tr{{border:1px solid var(--line);border-radius:12px;background:var(--bg2);margin-bottom:10px;padding:6px 0}}
 td{{border:0;padding:6px 14px}}
 td::before{{content:attr(data-h);display:block;font:600 10.5px/1.4 ui-monospace,Consolas,monospace;letter-spacing:.06em;text-transform:uppercase;color:var(--text3)}}
}}
@media print{{header,.toc,footer{{display:none}}body{{background:#fff;color:#000}}body::before{{display:none}}p,li,td{{color:#000}}.wrap{{display:block}}}}
</style>
<script>
(function(){{var l;try{{l=new URLSearchParams(location.search).get("lang")||localStorage.getItem("jc-lang")}}catch(e){{}}
if(l!=="en"&&l!=="cs")l=(navigator.language||"cs").slice(0,2)==="cs"||(navigator.language||"").slice(0,2)==="sk"?"cs":"en";
document.documentElement.dataset.lang=l;document.documentElement.lang=l;}})();
</script>
</head>
<body>
<header><div class="bar">
<a class="brand" href="https://justiceclient.online/"><i>J</i>JUSTICE</a>
<nav class="tabs">{tabs}</nav>
<div class="lang" role="group" aria-label="Language"><button data-set="cs">CZ</button><button data-set="en">EN</button></div>
</div></header>
<main class="wrap">{toc}{articles}</main>
<footer><span data-lang="cs">Nejde o oficiální produkt Minecraft. Není schválen společností Mojang ani Microsoft a není s nimi spojen.</span><span data-lang="en">Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.</span></footer>
<script>
(function(){{var t={{cs:"{title_cs}",en:"{title_en}"}},r=document.documentElement;
function set(l,save){{r.dataset.lang=l;r.lang=l;document.title=t[l]+" · Justice Client";
document.querySelectorAll(".lang button").forEach(function(b){{b.setAttribute("aria-pressed",b.dataset.set===l)}});
if(save)try{{localStorage.setItem("jc-lang",l)}}catch(e){{}}}}
document.querySelectorAll(".lang button").forEach(function(b){{b.onclick=function(){{set(b.dataset.set,1)}}}});
set(r.dataset.lang);}})();
</script>
</body>
</html>
"""

if __name__ == "__main__":
    (ROOT / "web").mkdir(exist_ok=True)
    for doc, cs, en in DOCS:
        (ROOT / "web" / f"{doc}.html").write_text(page(doc, cs, en), encoding="utf-8")
        print("wrote", f"web/{doc}.html")
