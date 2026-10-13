# Production Menu website

Small, dependency-free static site for GitHub Pages. HTML and CSS use a parchment/brass/navy palette and the user-supplied gameplay screenshot. All asset URLs are relative so the repository project path works. The download buttons point to the releases list, which includes experimental prereleases; they do not assume a stable release or promise an unavailable ZIP.

From the repository root:

```powershell
python tools/build-site.py
python -m http.server 4173 --bind 127.0.0.1 --directory .build/pages
```

Open http://127.0.0.1:4173. The builder stages only the website files and the screenshot from docs/images, then checks local links, section anchors, image alt text and the mobile viewport. No game files, research docs or development tools are deployed. The image has a full-size link; FAQs use native HTML disclosure controls and work without JavaScript.

## GitHub Pages

In the repository, choose **Settings → Pages → Build and deployment → Source → GitHub Actions**. Push main (with a website change) or run the **Website** workflow manually. Pull requests validate the staged site; main/manual runs also deploy it. The expected URL after a successful deployment is https://waldi.github.io/anno-1404-production-menu/.

If enabling Pages after the files were already pushed, run the workflow manually. Pages uses its own workflow and does not depend on creating a mod release tag. It updates when site source or the screenshot changes. Keep supported builds, controls and installation steps consistent with the root README and edition profiles. Asset copyrights are described in THIRD_PARTY_NOTICES.md.
