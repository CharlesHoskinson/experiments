"""Fetch reference URLs cited in README/ARCHITECTURE/GOALS via Scrapling.

Run once during the README overhaul to verify citations resolve and capture
titles + abstracts for cross-checking. Output is human-readable Markdown.
"""

from __future__ import annotations

import sys
from scrapling import Fetcher

URLS = [
    ("crypsinous", "https://eprint.iacr.org/2018/1132"),
    ("chronos", "https://eprint.iacr.org/2019/838"),
    ("minotaur", "https://eprint.iacr.org/2022/104"),
    ("starstream-repo", "https://github.com/LFDT-Nightstream/Starstream"),
    ("starstream-impl-plan", "https://github.com/LFDT-Nightstream/Starstream/blob/main/impl-plan.md"),
    ("cip-1694", "https://cips.cardano.org/cips/cip1694/"),
    ("eip-7524", "https://eips.ethereum.org/EIPS/eip-7524"),
    ("partner-chains", "https://github.com/input-output-hk/partner-chains"),
    ("fips-204", "https://csrc.nist.gov/pubs/fips/204/final"),
    ("fips-205", "https://csrc.nist.gov/pubs/fips/205/final"),
    ("fips-206-draft", "https://csrc.nist.gov/pubs/fips/206/ipd"),
    ("cli-pr-1350", "https://github.com/IntersectMBO/cardano-cli/pull/1350"),
]


def squeeze(text: str | None, n: int = 600) -> str:
    if not text:
        return ""
    return " ".join(text.split())[:n]


def first(page, sel):
    matches = page.css(sel)
    return matches[0] if matches else None


def fetch_one(slug: str, url: str) -> dict:
    try:
        page = Fetcher.get(url, timeout=30)
        title_node = first(page, "title")
        title = title_node.text if title_node else ""
        meta_desc = first(page, 'meta[name="description"]')
        og_desc = first(page, 'meta[property="og:description"]')
        body_excerpt = ""
        for sel in ("#abstract", ".abstract", "article", "#readme", "#repository-content"):
            node = first(page, sel)
            if node and node.text:
                body_excerpt = node.text
                break
        return {
            "slug": slug,
            "url": url,
            "status": page.status,
            "title": squeeze(str(title), 200),
            "meta_description": squeeze(
                (meta_desc.attrib.get("content") if meta_desc else "")
                or (og_desc.attrib.get("content") if og_desc else ""),
                400,
            ),
            "body_excerpt": squeeze(str(body_excerpt), 800),
            "error": None,
        }
    except Exception as exc:  # noqa: BLE001
        return {
            "slug": slug,
            "url": url,
            "status": None,
            "title": "",
            "meta_description": "",
            "body_excerpt": "",
            "error": f"{type(exc).__name__}: {exc}",
        }


def main() -> int:
    print("# Reference fetch results")
    print()
    print("| slug | status | title | error |")
    print("|---|---|---|---|")
    results: list[dict] = []
    for slug, url in URLS:
        sys.stderr.write(f"fetching {slug}...\n")
        sys.stderr.flush()
        r = fetch_one(slug, url)
        results.append(r)
        title = (r["title"] or "—").replace("|", "\\|")
        status = r["status"] or "ERR"
        err = (r["error"] or "—").replace("|", "\\|")
        print(f"| {slug} | {status} | {title} | {err} |")
    print()
    for r in results:
        print(f"## {r['slug']}")
        print(f"- url: {r['url']}")
        print(f"- status: {r['status']}")
        if r["title"]:
            print(f"- title: {r['title']}")
        if r["meta_description"]:
            print(f"- meta: {r['meta_description']}")
        if r["body_excerpt"]:
            print(f"- excerpt: {r['body_excerpt']}")
        if r["error"]:
            print(f"- ERROR: {r['error']}")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
