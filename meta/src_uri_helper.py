#!/usr/bin/env python3
from portage.dbapi.porttree import portdbapi
from portage.package.ebuild.config import config as econfig
import os, sys, json

# Small helper to get all SRC_URIS of an ebuild
# return a JSON object like:
#   "file": ["urls", ...]
#
# Usage:
# src_uri_helper.py path/to/ebuild

def main():
    # parse ebuild path
    ebuild = sys.argv[1]
    parts = ebuild.split("/")
    repo = "/".join(parts[:-3])
    cpv = parts[-3] + "/" + parts[-1].removesuffix(".ebuild")

    # get fetchmap from dbapi
    os.environ["PORTDIR_OVERLAY"] = repo
    dbapi = portdbapi()
    dbapi._set_porttrees([repo])
    fetchmap = dbapi.getFetchMap(cpv)

    # we need to manually expand mirror:// urls
    # TODO: check if this actually gets mirrors from PORTDIR_OVERLAY
    thirdpartymirrors = econfig().thirdpartymirrors()
    expanded_fetchmap = {}
    for file, uris in fetchmap.items():
        expanded_fetchmap[file] = []
        for uri in uris:
            if uri.startswith("mirror://"):
                mirror, path = uri.removeprefix("mirror://").split("/", 1)
                try:
                    expanded_fetchmap[file].extend([uri + "/" + path for uri in thirdpartymirrors[mirror]])
                except KeyError as e:
                    print(f"Error resolving mirror uri: {str(e)}", file=sys.stderr)
            else:
                expanded_fetchmap[file].append(uri)

    # return as json
    print(json.dumps(expanded_fetchmap))

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <path to ebuild>", file=sys.stderr)
        exit(1)
    try:
        main()
    except Exception as e:
        print(f"Error parsing ebuild {sys.argv[1]}: {str(e)}", file=sys.stderr)

