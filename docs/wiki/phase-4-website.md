# Phase 4 website integration

The website now builds the wiki at /wiki/ with 24 English articles. Article URLs are stable /wiki/article-id/ routes, with index.html files that also support direct-file navigation.

## Reader experience

- Header and footer wiki links on all 16 landing-page languages, explicitly marked EN.
- Six navigation sections, article breadcrumbs and a collapsible mobile article menu.
- Section contents links, previous/next articles and related articles.
- Local search for titles, headings, control names, model aliases and article text.
- Version labels for documented application behavior and captured screenshots.
- Visible draft status and pending full verification; no automatic promotion of article versions.
- Responsive reference tables, image captions and full-size screenshot links.
- Core reading/navigation available without JavaScript.

The wiki is English only. Language links return to the website's language selection rather than implying translated wiki articles exist.

## Source and build ownership

Application documentation remains the canonical authoring source. The website keeps a reviewed snapshot in wiki/content and wiki/snapshot.json. Import it with scripts/import-wiki.js, then build normally. A website build does not require the application checkout.

The snapshot preserves the documented target 0.2.29.3 and screenshot version 0.2.29.0. The renderer does not stamp articles with the newest application working version.

The existing static build and hosting configuration remain in use. SITE_ORIGIN enables wiki canonical URLs and sitemap entries alongside the localized landing pages.

## Validation on 2026-10-30

Eight automated checks passed: landing-page localization/navigation, direct-file asset references, translation fallback and language switching, all wiki links/assets/anchors, search, safe Markdown rendering, and canonical/sitemap/language-link integration.

Browser checks covered the wiki overview, search to the unlock-value article section, phone-width article layout, missing-image detection, the mobile landing-page wiki link and horizontal overflow. Desktop and mobile preview images were retained in the website workspace.

The browser's URL policy does not allow file-scheme navigation, so direct-file behavior was checked through static references and the no-fetch search implementation, not through browser file navigation.

## Publication work for phase 5

No website deployment or GitHub source publication was performed during phase 4. The intended public wiki address is https://achimeld.com/wiki/. It returned HTTP 404 on 2026-10-30 and must be checked after deployment.

Complete content/runtime acceptance and remaining illustrations, confirm the production origin, deploy the reviewed website and verify its public routes. Then publish the README Wiki link with the source update. Preserve the public checkout and existing source-publication workflow. See [Wiki maintenance](../wiki-maintenance.md).

