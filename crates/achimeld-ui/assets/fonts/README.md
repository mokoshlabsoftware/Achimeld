# Bundled Noto CJK fonts

Unmodified Noto Sans CJK **2.004** static OpenType collections from
[notofonts/noto-cjk](https://github.com/notofonts/noto-cjk/tree/523d033d6cb47f4a80c58a35753646f5c3608a78),
tag `Sans2.004`, commit `523d033d6cb47f4a80c58a35753646f5c3608a78`.
Copyright 2014–2021 Adobe (http://www.adobe.com/).
Distributed under the [SIL Open Font License 1.1](OFL.txt).

| File | Upstream path | SHA-256 |
| --- | --- | --- |
| NotoSansCJK-Regular.ttc | Sans/OTC/NotoSansCJK-Regular.ttc | b76b0433203017ca80401b2ee0dd69350349871c4b19d504c34dbdd80541690a |
| NotoSansCJK-Bold.ttc | Sans/OTC/NotoSansCJK-Bold.ttc | faa5f3656a78b2e2d450d27fe8382c778bc2b6bb5ea29c986664a6a435056ceb |
| OFL.txt | LICENSE | 6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2 |

Each collection includes JP (index 0), KR (1), SC (2), TC (3), HK (4),
then their Mono faces (5–9). The UI uses JP, KR, SC or TC according to its
interface language, with SC as the fallback for other languages. All faces
retain full CJK coverage, including mixed-language project text. Normal,
bold and monospace UI families keep their existing Latin faces first and
append the corresponding Noto face. Text-layer artwork uses the separate
host-owned system-font renderer.

The collections are embedded in the executable as shared static bytes;
language changes select collection indices without loading font files.
Builds require no font downloads and users need no CJK language packs.
