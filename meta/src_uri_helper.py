#!/usr/bin/env python3
from portage.dbapi.porttree import portdbapi
import os, sys, json

# Small helper to get all SRC_URIS of an ebuild
# return a JSON object like:
#   "file": ["urls", ...]
#
# Usage:
# src_uri_helper.py path/to/ebuild

def main():
    ebuild = sys.argv[1]
    parts = ebuild.split("/")
    repo = "/".join(parts[:-3])
    cpv = parts[-3] + "/" + parts[-1].removesuffix(".ebuild")

    os.environ["PORTDIR_OVERLAY"] = repo
    api = portdbapi()
    api._set_porttrees([repo])
    fetchmap = api.getFetchMap(cpv)
    print(json.dumps(fetchmap))

if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"Error parsing ebuild {sys.argv[1]}: {e}", file=sys.stderr)

