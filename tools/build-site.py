"""Stage and validate the dependency-free GitHub Pages site."""
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlsplit
import shutil

repo = Path(__file__).resolve().parents[1]
source = repo / 'site'
output = repo / '.build/pages'
output.mkdir(parents=True, exist_ok=True)
(output / 'assets').mkdir(exist_ok=True)
for relative in ('index.html', 'style.css', 'assets/crest.svg'):
    shutil.copyfile(source / relative, output / relative)
shutil.copyfile(repo / 'docs/images/production-menu.png', output / 'assets/production-menu.png')
(output / '.nojekyll').write_text('')
expected = {'index.html', 'style.css', 'assets/crest.svg', 'assets/production-menu.png', '.nojekyll'}
actual = {file.relative_to(output).as_posix() for file in output.rglob('*') if file.is_file()}
assert actual == expected, 'Unexpected files in site staging; inspect before deploying'

class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.links = []
        self.viewport = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, f'Duplicate ID: {attrs["id"]}'
            self.ids.add(attrs['id'])
        if tag == 'img':
            assert attrs.get('alt'), 'Image requires descriptive alt text'
        if tag == 'meta' and attrs.get('name') == 'viewport':
            self.viewport = True
        for attribute in ('href', 'src'):
            if attrs.get(attribute):
                self.links.append(attrs[attribute])

page = Page()
page.feed((output / 'index.html').read_text(encoding='utf-8'))
assert page.viewport, 'Missing mobile viewport'
for link in page.links:
    parsed = urlsplit(link)
    if parsed.scheme or parsed.netloc:
        assert parsed.scheme == 'https', f'Unexpected remote link: {link}'
        continue
    if parsed.path:
        assert not parsed.path.startswith('/'), f'Project Pages requires relative assets: {link}'
        target = (output / parsed.path).resolve()
        assert target.is_relative_to(output.resolve()) and target.is_file(), f'Missing asset: {link}'
    if parsed.fragment:
        assert parsed.fragment in page.ids, f'Missing section: {link}'
print(f'Site staged at {output}; local links, anchors, alt text and viewport verified.')
