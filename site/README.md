# The Roomtone website

A small static site: plain HTML, one stylesheet and one script. No build step, no framework, no cookies, no analytics, no outside fonts. Open `index.html` through any web server (`python3 -m http.server --directory site`) to see it.

| File | What |
| --- | --- |
| `index.html` | Home: what it is, why, features, screenshots, assistants and scripts, privacy, questions |
| `download.html` | Downloads for each computer, first-run steps, how to check a download |
| `privacy.html` | What the app and this website do and don't send |
| `style.css`, `site.js` | Look (light and dark follow the computer) and the version and download-link fill-in |
| `img/` | Screenshots (from the demo system, resized from `docs/images/`), and the app icon made with `roomtone --write-icon` |

## How it behaves

- Every link is relative, so it works at a domain's root or under `https://chrispygee.github.io/Roomtone/` until there's a domain.
- `site.js` asks `api.github.com` for the latest release and fills in the version and the download buttons. Without JavaScript, or if GitHub can't be reached, the buttons go to the releases page. Nothing else is fetched. The privacy page says so.
- Nothing about prices or paid apps is on the site on purpose. The focus is the free desktop app.
- The text repeats claims from the README and `SPEC.md`. When those change, change the site too (the supported systems, languages and features).
- Screenshots are demo-system only: never real room names, addresses or tokens.

## Putting it online

Not done yet. Two ways, both free:

1. **GitHub Pages** from this repository: *Settings › Pages*, deploy from a branch or from a workflow, folder `site/`. The address is `https://chrispygee.github.io/Roomtone/` until a domain is added (add it under *Pages › Custom domain*, and a `CNAME` file in `site/`).
2. Any static host (Cloudflare Pages, Netlify and so on): publish the `site/` folder.

When there's a domain, also set `og:image` and `og:url` in each page's head to full addresses (for example `https://<domain>/img/social.jpg`) so shared links show a picture.

## Choosing a domain

Checked on 2026-11-21 with DNS and WHOIS lookups. This only shows what *looks* unregistered; confirm at a registrar before getting attached to one.

| Looks taken | `roomtone.com`, `.app`, `.io`, `.dev`, `.fm`, `.net`, `.org`, `.co`, `.cc`, `.xyz`, `.live`, `.space`, `.site`, `.studio`, `.tools`, `.tech`, `getroomtone.com`, `roomtoneapp.com`, `tryroomtone.com` |
| Looked free | `roomtone.audio`, `roomtone.music`, `roomtone.me`, `roomtone.sh`, `useroomtone.com` |

Things to weigh:

- **`roomtone.audio` or `roomtone.music`** say what it is, and are exactly the name. New-style endings often cost more each year, and some are priced as "premium"; check the renewal price, not the first-year offer.
- **`useroomtone.com`** is a normal `.com`, which people trust and type correctly, at the cost of the extra word. (`get`, `try` and `app` versions are taken.)
- **`roomtone.sh`** and `.me` are cheap but read as developer or personal sites.
- **Don't put "sonos" in the name.** It's someone else's trademark and the project already says it isn't affiliated.
- **Check the name itself** at a trademark register (UK IPO, USPTO, EUIPO) before spending on branding: `roomtone.com` belongs to someone, and an existing business under that name matters more than the domain.
- Register the one you choose plus the other likely endings you can afford, and set one to redirect to the other. Turn on registrar privacy and auto-renew.
