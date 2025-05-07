#!/usr/bin/env bash

set -euo pipefail

rm -r /tmp/portcache/distfiles
mkdir /tmp/portcache/distfiles
curl 'http://127.0.0.1:8000/distfiles/00/harfbuzz-10.4.0.tar.xz' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/00/harfbuzz-10.4.0.tar.xz' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/00/harfbuzz-10.4.0.tar.xz' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/00/net-imap-0.5.7.tar.gz' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/00/reline-0.6.1.tar.gz' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/f3/badfile' > /dev/null 2>&1 &
curl 'http://127.0.0.1:8000/distfiles/00/baddigest' > /dev/null 2>&1 &
