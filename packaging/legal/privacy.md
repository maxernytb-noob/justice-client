---
doc: privacy
version: 2026-10-29
---

<!-- lang:cs -->

# Zásady ochrany osobních údajů

> **Upozornění k návrhu:** Tento text je návrh připravený bez právní kontroly a není právní radou. Před vydáním ho nechte zkontrolovat advokátem se znalostí GDPR. Zkontrolujte také, že popis odpovídá skutečnému chování launcheru a serveru.

**Verze:** 2026-10-29 · **Účinnost od:** [DOPLNIT: datum účinnosti] · **Závazné znění:** české.

Tyto zásady vysvětlují, jaké osobní údaje zpracováváme, když používáte Justice Client, Justice Mod, službu Justice Social a web justiceclient.online (dále „**Služby**“), proč, jak dlouho a jaká máte práva. Řídíme se nařízením (EU) 2016/679 (GDPR) a zákonem č. 110/2019 Sb., o zpracování osobních údajů.

## 1. Správce

Správcem osobních údajů je fyzická osoba **[DOPLNIT: jméno a příjmení provozovatele]**, adresa pro doručování **[DOPLNIT: adresa pro doručování]**, e-mail **[DOPLNIT: e-mail pro ochranu osobních údajů]** (dále „**my**“). Pověřence pro ochranu osobních údajů nemáme jmenovaného, protože nám to zákon neukládá; se vším se obracejte na uvedený e-mail.

## 2. Stručně

- **Launcher bez účtu** posílá na náš server jen to, co je nutné k aktualizacím a stažení obrázků (vaše IP adresa a technické údaje požadavku).
- **Účet Justice Social** potřebuje vaše herní jméno a UUID; u offline účtu také heslo, které ukládáme jen jako bezpečný otisk (Argon2).
- Zprávy, přátele, profil, skiny a klipy ukládáme, abychom vám mohli poskytovat sociální funkce.
- **Crash logy a logy hry zůstávají jen ve vašem počítači.** Nic nesbíráme automaticky, nepoužíváme analytické nástroje, reklamu ani sledovací cookies a údaje neprodáváme.
- Integrace hudebního přehrávače (Sonolith apod.) a Discord Rich Presence fungují **jen lokálně** ve vašem počítači.

## 3. Jaké údaje zpracováváme, proč a na jakém základě

### 3.1 Launcher a web bez účtu

| Údaje | Účel | Právní základ | Doba uchování |
|---|---|---|---|
| IP adresa, čas, požadovaná adresa, User-Agent (verze launcheru / prohlížeče) v logu webového serveru | doručení aktualizací, obrázků a webu; zabezpečení a řešení útoků | oprávněný zájem – čl. 6 odst. 1 písm. f) GDPR (bezpečný provoz) | [DOPLNIT: doba uchování logů nginx, doporučeno max. 14 dní] |
| Volba jazyka webu (`jc-lang` v localStorage prohlížeče) | zapamatování jazyka | nezbytné pro službu, kterou si žádáte; zůstává jen ve vašem prohlížeči | dokud ji nesmažete |

Launcher si v počítači ukládá svá data (nastavení, instance, přihlašovací tokeny účtů, klipy, logy, crash logy, nalezená „tajemství“). Ta k nám **neodesílá** – zpracováváte je vy ve svém zařízení.

### 3.2 Účet Justice Social

| Údaje | Účel | Právní základ | Doba uchování |
|---|---|---|---|
| UUID a herní jméno Minecraftu, typ účtu (Microsoft / offline), datum vytvoření, čas poslední aktivity | vytvoření a provoz účtu, vyhledání hráče přáteli | plnění smlouvy – čl. 6 odst. 1 písm. b) | po dobu existence účtu |
| U offline účtu: jméno a heslo uložené jako otisk Argon2 (heslo samotné nikdy) | přihlášení | plnění smlouvy | po dobu existence účtu |
| Přihlašovací relace (náhodný token) | udržení přihlášení | plnění smlouvy | do odhlášení, změny hesla nebo zrušení účtu |
| Profil: bio, status, banner, zvolený plášť, statistiky a úspěchy, které launcher spočítá (např. odehraný čas) | zobrazení profilu přátelům a v žebříčku | plnění smlouvy | do změny nebo zrušení účtu |
| Uložený skin offline účtu (obrázek PNG, varianta modelu) | zobrazení skinu ve hře ostatním hráčům Justice | plnění smlouvy | do resetu skinu nebo zrušení účtu |
| Přátelé a žádosti o přátelství | funkce přátel | plnění smlouvy | do odebrání přítele nebo zrušení účtu |
| Zprávy v chatu (text, typ, metadata jako pozvánky, čas, přečtení) | doručení zpráv a historie konverzace | plnění smlouvy | po dobu existence účtu; [DOPLNIT: rozhodnout, zda zprávy mazat automaticky, např. po 12 měsících] |
| Soubory poslané v chatu (obrázky, videa) | doručení příjemci | plnění smlouvy | automaticky se mažou po **30 dnech** |
| Stav online (presence: online/offline, verze hry, případně server, na kterém hrajete) | zobrazení stavu přátelům | plnění smlouvy | jen v operační paměti serveru po dobu připojení; neukládá se |
| Party (členové, pozvánky, warp na server) | společné hraní | plnění smlouvy | jen v paměti po dobu existence party |
| Sdílené světy přes relay: název světa, verze hry a loaderu, seznam pozvaných přátel; herní data předávaná mezi hostitelem a hostem | připojení přátel ke světu bez otevírání portů | plnění smlouvy | metadata jen v paměti po dobu sdílení; herní provoz **jen přeposíláme a neukládáme** |
| Technické logy serveru (např. záznam o nahrání souboru: název, velikost, UUID) | bezpečnost, řešení chyb, prevence zneužití | oprávněný zájem – čl. 6 odst. 1 písm. f) | [DOPLNIT: doba uchování logů Dockeru, doporučeno max. 30 dní] |

Při přihlášení účtem Microsoft ověřujeme, že účet vlastníte, dotazem na server Mojangu (`sessionserver.mojang.com`), kterému předáváme vaše herní jméno a jednorázový identifikátor. Vaše heslo ani tokeny Microsoftu na náš server nikdy nepřicházejí.

### 3.3 Zveřejněné klipy (připravovaná funkce)

| Údaje | Účel | Právní základ | Doba uchování |
|---|---|---|---|
| Video klipu (max. cca 60 s / 100 MB) a náhled, popis, hashtagy, označení hráčů, čas zveřejnění, autor | zveřejnění klipu ostatním uživatelům | plnění smlouvy – čl. 6 odst. 1 písm. b) | dokud klip nesmažete vy nebo my; po zrušení účtu se smaže |
| Lajky (kdo dal lajk kterému klipu), případně počet zhlédnutí | zobrazení lajků a oblíbenosti | plnění smlouvy | po dobu existence klipu nebo účtu |
| Označení vás v cizím klipu (vaše herní jméno) | upozornění a propojení s profilem | oprávněný zájem autora klipu a náš – čl. 6 odst. 1 písm. f); označení můžete odebrat | po dobu existence klipu nebo do odebrání označení |

Klip může zachycovat i jiné hráče (jejich jména, skiny, hlas z hlasového chatu). Za to, že má autor k jejich zveřejnění oprávnění, odpovídá autor. Pokud jste v cizím klipu a nesouhlasíte s tím, napište nám nebo klip nahlaste.

### 3.4 Pomocník pro modpacky s AI (připravovaná funkce)

| Údaje | Účel | Právní základ | Doba uchování |
|---|---|---|---|
| Text vašeho dotazu, kontext potřebný k odpovědi (např. verze hry, loader, seznam modů instance), UUID kvůli limitu dotazů | odpověď na dotaz | plnění smlouvy – čl. 6 odst. 1 písm. b) | na našem serveru se obsah dotazů **neukládá**, uchováváme jen počítadlo dotazů na 24 hodin [DOPLNIT: ověřit po implementaci proxy] |

Dotazy přeposíláme přes náš server poskytovateli jazykového modelu (viz bod 5). Do dotazů nevkládejte osobní údaje.

### 3.5 Nahlášení, moderace a komunikace s námi

| Údaje | Účel | Právní základ | Doba uchování |
|---|---|---|---|
| Obsah nahlášení, nahlášený obsah, jméno a e-mail oznamovatele, rozhodnutí a jeho odůvodnění, odvolání | vyřízení nahlášení podle DSA, ochrana uživatelů, obrana právních nároků | právní povinnost – čl. 6 odst. 1 písm. c) (nařízení DSA) a oprávněný zájem – písm. f) | 12 měsíců od rozhodnutí (u probíhajícího sporu do jeho skončení) |
| E-mailová komunikace (adresa, obsah) | odpověď na váš dotaz, žádost o uplatnění práv | oprávněný zájem; u žádostí podle GDPR právní povinnost | 3 roky od vyřízení |

### 3.6 Co nezpracováváme

Nepoužíváme analytiku, reklamní ani sledovací nástroje, nevytváříme profily pro marketing a neprovádíme automatizované rozhodování s právními nebo podobně závažnými účinky ve smyslu čl. 22 GDPR. Obsah ani údaje neprodáváme.

## 4. Údaje, které jdou z vašeho počítače přímo třetím stranám

Některé funkce launcheru komunikují **přímo** z vašeho počítače se službami třetích stran. Ty pak zpracovávají vaši IP adresu a údaje požadavku jako samostatní správci podle svých vlastních zásad. Od nás tyto údaje nedostávají:

- **Microsoft / Xbox Live / Mojang** – přihlášení účtem Microsoft, stažení hry, skiny a profily hráčů (Mojang API), obrázky z Minecraft Launcher API;
- **Modrinth** (Rokit LLC, USA) a **CurseForge** (Overwolf) – vyhledávání a stahování modů a modpacků;
- **Discord** – Rich Presence předává informace o hře aplikaci Discord ve vašem počítači; Discord je pak zobrazí podle vašeho nastavení. Funkci lze vypnout v Nastavení launcheru;
- **Fabric, NeoForge, Forge** a další zdroje knihoven a loaderů;
- **herní servery**, na které se připojíte;
- **hudební přehrávač** (např. Sonolith) – launcher jen čte informace o přehrávání ze systému (Windows SMTC / Linux MPRIS) ve vašem počítači, nic se neodesílá.

## 5. Příjemci a zpracovatelé

| Příjemce | Role | Údaje | Umístění |
|---|---|---|---|
| **[DOPLNIT: poskytovatel VPS – název a sídlo]** | zpracovatel – hosting serveru, na kterém běží web, aktualizace a Justice Social | všechny údaje uložené na serveru | [DOPLNIT: stát datového centra] |
| **[DOPLNIT: registrátor/DNS – např. Namecheap]** | poskytovatel domény a DNS | jen DNS dotazy, nikoli obsah | [DOPLNIT: země] |
| **[DOPLNIT: poskytovatel LLM – Groq, Inc. (USA) nebo Google (Gemini API)]** | zpracovatel / poskytovatel – odpovědi pomocníka s AI | text dotazu a kontext (bez vašeho jména, UUID a IP adresy) | USA |
| **Mojang / Microsoft** | samostatný správce – ověření vlastnictví účtu Microsoft | herní jméno, jednorázový identifikátor, IP adresa našeho serveru | EU / USA |
| Ostatní uživatelé Služeb | podle vámi zvolené viditelnosti (přátelé, party, veřejně) | jméno, profil, skin, plášť, zprávy, klipy | — |
| Orgány veřejné moci | jen pokud to ukládá zákon (např. soud, policie) | v nezbytném rozsahu | — |

Se zpracovateli máme (nebo uzavřeme) smlouvu o zpracování podle čl. 28 GDPR, typicky v podobě jejich standardních podmínek.

## 6. Předávání mimo EU / EHP

Pokud se údaje dostanou mimo EU/EHP (zejména k poskytovateli LLM v USA), děje se tak na základě rozhodnutí Evropské komise o odpovídající ochraně (EU–US Data Privacy Framework, je-li příjemce certifikován) nebo standardních smluvních doložek podle čl. 46 GDPR. Kopii záruk vám na požádání poskytneme. [DOPLNIT: ověřit certifikaci DPF zvoleného poskytovatele LLM.]

## 7. Jak údaje chráníme

Komunikace se serverem je šifrovaná (HTTPS / WSS), hesla offline účtů ukládáme jen jako otisky Argon2, aktualizace launcheru jsou digitálně podepsané, přístup k serveru je omezen na provozovatele a chráněn klíči. Žádné zabezpečení ale není dokonalé; o porušení zabezpečení, které by pro vás představovalo vysoké riziko, vás budeme informovat podle čl. 34 GDPR.

## 8. Vaše práva

Máte právo:

- **na přístup** – zjistit, jaké údaje o vás zpracováváme, a dostat jejich kopii;
- **na opravu** nepřesných údajů;
- **na výmaz** („být zapomenut“) – zejména zrušení účtu a smazání souvisejících dat;
- **na omezení zpracování**;
- **na přenositelnost** – dostat údaje, které jste nám poskytli, ve strojově čitelném formátu (JSON);
- **vznést námitku** proti zpracování založenému na oprávněném zájmu;
- **odvolat souhlas**, pokud je zpracování založeno na souhlasu (odvolání nemá vliv na předchozí zpracování);
- **podat stížnost** u dozorového úřadu: **Úřad pro ochranu osobních údajů**, Pplk. Sochora 27, 170 00 Praha 7, https://uoou.gov.cz – nebo u úřadu ve státě vašeho bydliště či pracoviště.

Práva uplatníte e-mailem na **[DOPLNIT: e-mail pro ochranu osobních údajů]**. Odpovíme bez zbytečného odkladu, nejpozději do 1 měsíce (ve složitých případech lze lhůtu prodloužit o další 2 měsíce, o čemž vás informujeme). Abychom nevydali údaje někomu jinému, můžeme ověřit, že jste držitelem účtu (např. zprávou v launcheru přihlášenému účtu). Vyřízení je zdarma.

## 9. Zrušení účtu

O zrušení účtu požádejte e-mailem (nebo v launcheru, až tam bude tato funkce). Do **30 dní** smažeme účet, profil, skin, přátele, relace, vaše klipy a lajky a zprávy, které jste odeslali [DOPLNIT: rozhodnout, zda mazat i zprávy, které vám poslali jiní – doporučeno ano, protože jsou ve vaší konverzaci]. Zálohy (pokud existují) se přepíší nejpozději do **[DOPLNIT: doba rotace záloh, doporučeno 30 dní]**. Údaje, které musíme uchovat ze zákona nebo k obraně právních nároků (např. záznamy o moderaci), uchováme jen po dobu uvedenou výše.

## 10. Děti

Služby s účtem jsou určeny osobám od 15 let; mladší je smějí používat jen se souhlasem zákonného zástupce (čl. 8 GDPR a § 7 zákona č. 110/2019 Sb.). Pokud zjistíme, že jsme bez takového souhlasu zpracovali údaje dítěte mladšího 15 let, údaje smažeme. Zákonný zástupce nás může kontaktovat na uvedeném e-mailu.

## 11. Cookies a localStorage

Web **justiceclient.online nepoužívá cookies** ani analytické či reklamní nástroje třetích stran a nenačítá písma ani skripty z cizích serverů. Do localStorage prohlížeče ukládá jen zvolený jazyk (`jc-lang`); tento údaj nikam neodesíláme. Protože jde o nezbytné uložení na vaši žádost, nevyžaduje souhlas.

Rozhraní launcheru běží ve webovém zobrazení systému (WebView2 / WebKitGTK) a do jeho localStorage ukládá nastavení rozhraní a stav „tajemství“. Tyto údaje zůstávají ve vašem počítači.

## 12. Změny těchto zásad

Zásady můžeme aktualizovat. O podstatných změnách vás budeme informovat v launcheru a na webu; pokud to bude nutné, požádáme vás o nové odsouhlasení. Datum verze je uvedeno nahoře.

<!-- lang:en -->

# Privacy Policy

> **Draft notice:** This text is a draft prepared without legal review and is not legal advice. Before release, have it reviewed by a lawyer familiar with the GDPR. Also check that the description matches how the launcher and server actually behave.

**Version:** 2026-10-29 · **Effective from:** [DOPLNIT: effective date] · **Binding version:** Czech.

This policy explains what personal data we process when you use Justice Client, Justice Mod, the Justice Social service and the justiceclient.online website (the "**Services**"), why, for how long and what rights you have. We comply with Regulation (EU) 2016/679 (GDPR) and Czech Act No. 110/2019 Coll. on personal data processing.

## 1. Controller

The controller is a natural person, **[DOPLNIT: operator's full name]**, address for correspondence **[DOPLNIT: address for correspondence]**, e-mail **[DOPLNIT: privacy e-mail]** ("**we**"). We have not appointed a data protection officer because the law does not require it; please contact us at the e-mail above.

## 2. In short

- **The launcher without an account** sends our server only what is needed for updates and images (your IP address and technical request data).
- **A Justice Social account** needs your in-game name and UUID; for an offline account also a password, which we store only as a secure hash (Argon2).
- We store messages, friends, profiles, skins and clips so that we can provide social features.
- **Crash logs and game logs stay on your computer only.** We collect nothing automatically, use no analytics, advertising or tracking cookies and do not sell data.
- The music player integration (Sonolith etc.) and Discord Rich Presence work **only locally** on your computer.

## 3. What data we process, why and on what basis

### 3.1 Launcher and website without an account

| Data | Purpose | Legal basis | Retention |
|---|---|---|---|
| IP address, time, requested URL, User-Agent (launcher / browser version) in the web server log | delivering updates, images and the website; security and handling attacks | legitimate interest – Art. 6(1)(f) GDPR (secure operation) | [DOPLNIT: nginx log retention, recommended max. 14 days] |
| Website language choice (`jc-lang` in browser localStorage) | remembering the language | necessary for the service you request; stays only in your browser | until you delete it |

The launcher stores its data on your computer (settings, instances, account sign-in tokens, clips, logs, crash logs, discovered "secrets"). It does **not send** them to us – you process them on your own device.

### 3.2 Justice Social account

| Data | Purpose | Legal basis | Retention |
|---|---|---|---|
| Minecraft UUID and in-game name, account type (Microsoft / offline), creation date, last activity time | creating and running the account, letting friends find you | performance of contract – Art. 6(1)(b) | for the life of the account |
| For an offline account: name and password stored as an Argon2 hash (never the password itself) | sign-in | performance of contract | for the life of the account |
| Sign-in sessions (random token) | keeping you signed in | performance of contract | until sign-out, password change or account closure |
| Profile: bio, status, banner, chosen cape, statistics and achievements computed by the launcher (e.g. play time) | showing your profile to friends and on the leaderboard | performance of contract | until changed or the account is closed |
| Stored skin of an offline account (PNG image, model variant) | showing the skin in game to other Justice players | performance of contract | until skin reset or account closure |
| Friends and friend requests | friends feature | performance of contract | until a friend is removed or the account is closed |
| Chat messages (text, type, metadata such as invitations, time, read state) | delivering messages and conversation history | performance of contract | for the life of the account; [DOPLNIT: decide whether to delete messages automatically, e.g. after 12 months] |
| Files sent in chat (images, videos) | delivery to the recipient | performance of contract | deleted automatically after **30 days** |
| Online status (presence: online/offline, game version, possibly the server you play on) | showing your status to friends | performance of contract | only in server memory while connected; not stored |
| Party (members, invitations, warp to a server) | playing together | performance of contract | only in memory while the party exists |
| Shared worlds via relay: world name, game and loader version, list of invited friends; game data passed between host and guest | letting friends join a world without opening ports | performance of contract | metadata only in memory while sharing; game traffic is **only forwarded, never stored** |
| Server technical logs (e.g. a record of a file upload: name, size, UUID) | security, troubleshooting, abuse prevention | legitimate interest – Art. 6(1)(f) | [DOPLNIT: Docker log retention, recommended max. 30 days] |

When you sign in with a Microsoft account, we verify that you own it by querying Mojang's server (`sessionserver.mojang.com`) with your in-game name and a one-time identifier. Your Microsoft password and tokens never reach our server.

### 3.3 Published clips (upcoming feature)

| Data | Purpose | Legal basis | Retention |
|---|---|---|---|
| Clip video (max. approx. 60 s / 100 MB) and thumbnail, description, hashtags, player tags, publication time, author | publishing the clip to other users | performance of contract – Art. 6(1)(b) | until you or we delete the clip; deleted when the account is closed |
| Likes (who liked which clip), possibly view count | showing likes and popularity | performance of contract | for the life of the clip or account |
| Being tagged in someone else's clip (your in-game name) | notification and linking to your profile | legitimate interest of the clip author and ours – Art. 6(1)(f); you can remove the tag | for the life of the clip or until the tag is removed |

A clip may capture other players (their names, skins, voice from voice chat). The author is responsible for having the right to publish them. If you appear in someone else's clip and object, write to us or report the clip.

### 3.4 AI modpack helper (upcoming feature)

| Data | Purpose | Legal basis | Retention |
|---|---|---|---|
| Text of your question, context needed for the answer (e.g. game version, loader, list of mods in the instance), UUID for rate limiting | answering the question | performance of contract – Art. 6(1)(b) | question content is **not stored** on our server; we only keep a question counter for 24 hours [DOPLNIT: verify once the proxy is implemented] |

Questions are forwarded through our server to a language-model provider (see section 5). Do not put personal data into your questions.

### 3.5 Reports, moderation and communication with us

| Data | Purpose | Legal basis | Retention |
|---|---|---|---|
| Content of the report, reported content, reporter's name and e-mail, the decision and its reasons, appeals | handling reports under the DSA, protecting users, defending legal claims | legal obligation – Art. 6(1)(c) (DSA Regulation) and legitimate interest – (f) | 12 months from the decision (until the end of any ongoing dispute) |
| E-mail communication (address, content) | replying to your question or request to exercise rights | legitimate interest; for GDPR requests a legal obligation | 3 years after it is handled |

### 3.6 What we do not process

We use no analytics, advertising or tracking tools, build no marketing profiles and make no automated decisions with legal or similarly significant effects within the meaning of Art. 22 GDPR. We do not sell content or data.

## 4. Data sent from your computer directly to third parties

Some launcher features communicate **directly** from your computer with third-party services. They then process your IP address and request data as independent controllers under their own policies. They do not receive this data from us:

- **Microsoft / Xbox Live / Mojang** – Microsoft sign-in, game downloads, player skins and profiles (Mojang API), images from the Minecraft Launcher API;
- **Modrinth** (Rokit LLC, USA) and **CurseForge** (Overwolf) – searching and downloading mods and modpacks;
- **Discord** – Rich Presence passes game information to the Discord app on your computer; Discord then displays it according to your settings. You can turn the feature off in the launcher Settings;
- **Fabric, NeoForge, Forge** and other sources of libraries and loaders;
- **game servers** you connect to;
- **music player** (e.g. Sonolith) – the launcher only reads playback information from the system (Windows SMTC / Linux MPRIS) on your computer; nothing is sent.

## 5. Recipients and processors

| Recipient | Role | Data | Location |
|---|---|---|---|
| **[DOPLNIT: VPS provider – name and registered office]** | processor – hosting of the server running the website, updates and Justice Social | all data stored on the server | [DOPLNIT: data centre country] |
| **[DOPLNIT: registrar/DNS – e.g. Namecheap]** | domain and DNS provider | DNS queries only, not content | [DOPLNIT: country] |
| **[DOPLNIT: LLM provider – Groq, Inc. (USA) or Google (Gemini API)]** | processor / provider – AI helper answers | question text and context (without your name, UUID or IP address) | USA |
| **Mojang / Microsoft** | independent controller – verifying Microsoft account ownership | in-game name, one-time identifier, our server's IP address | EU / USA |
| Other users of the Services | according to the visibility you choose (friends, party, public) | name, profile, skin, cape, messages, clips | — |
| Public authorities | only where required by law (e.g. a court, police) | to the extent necessary | — |

We have (or will conclude) data processing agreements with processors under Art. 28 GDPR, typically in the form of their standard terms.

## 6. Transfers outside the EU / EEA

Where data leaves the EU/EEA (in particular to the LLM provider in the USA), this happens on the basis of a European Commission adequacy decision (the EU–US Data Privacy Framework, if the recipient is certified) or standard contractual clauses under Art. 46 GDPR. We will provide a copy of the safeguards on request. [DOPLNIT: verify the DPF certification of the chosen LLM provider.]

## 7. How we protect data

Communication with the server is encrypted (HTTPS / WSS), offline account passwords are stored only as Argon2 hashes, launcher updates are digitally signed, and server access is limited to the operator and protected by keys. No security is perfect, however; we will inform you of a breach that poses a high risk to you in accordance with Art. 34 GDPR.

## 8. Your rights

You have the right:

- **of access** – to find out what data we process about you and receive a copy;
- **to rectification** of inaccurate data;
- **to erasure** ("to be forgotten") – in particular closing your account and deleting related data;
- **to restriction of processing**;
- **to data portability** – to receive data you gave us in a machine-readable format (JSON);
- **to object** to processing based on legitimate interest;
- **to withdraw consent** where processing is based on consent (without affecting earlier processing);
- **to lodge a complaint** with the supervisory authority: **Úřad pro ochranu osobních údajů** (Czech Office for Personal Data Protection), Pplk. Sochora 27, 170 00 Prague 7, https://uoou.gov.cz – or with the authority in the country where you live or work.

Exercise your rights by e-mail to **[DOPLNIT: privacy e-mail]**. We will reply without undue delay, at the latest within 1 month (in complex cases this may be extended by 2 further months, of which we will inform you). To avoid disclosing data to someone else, we may verify that you hold the account (e.g. via a message in the launcher to the signed-in account). It is free of charge.

## 9. Closing your account

Ask us to close your account by e-mail (or in the launcher once the feature is there). Within **30 days** we will delete the account, profile, skin, friends, sessions, your clips and likes and the messages you sent [DOPLNIT: decide whether to also delete messages others sent you – recommended yes, as they are in your conversation]. Backups (if any) are overwritten within **[DOPLNIT: backup rotation period, recommended 30 days]**. Data we must keep by law or to defend legal claims (e.g. moderation records) is kept only for the period stated above.

## 10. Children

Services with an account are intended for people aged 15 and over; younger people may use them only with a legal guardian's consent (Art. 8 GDPR and Section 7 of Act No. 110/2019 Coll.). If we find we have processed data of a child under 15 without such consent, we will delete it. A guardian may contact us at the e-mail above.

## 11. Cookies and localStorage

The **justiceclient.online website uses no cookies** and no third-party analytics or advertising tools, and loads no fonts or scripts from other servers. It stores only your chosen language (`jc-lang`) in browser localStorage; this value is never sent anywhere. As this is storage strictly necessary at your request, it does not require consent.

The launcher interface runs in the system web view (WebView2 / WebKitGTK) and stores interface settings and the state of "secrets" in its localStorage. This data stays on your computer.

## 12. Changes to this policy

We may update this policy. We will inform you of material changes in the launcher and on the website; where necessary we will ask you to accept it again. The version date is shown at the top.
