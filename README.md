# PORTage CACHE

Cache server for portage distfiles - for those who maintain multiple Gentoo machines
or just want this "because it's cool" ^^

**THIS IS VERY MUCH STILL A WORK IN PROGRESS**

## Goals

- AdHoc caching of distfiles (only store what you use)
- Load balancing across source mirrors
- Fetch from `SRC_URI` directly if not mirrored

## TODO

- [x] AdHoc fetching from Gentoo mirrors
- [ ] Look up and fetch from `SRC_URI`  
      Look through all `Manifest` files in ebuild trees to find package wanting `file`.
      Then `ebuild setup` and grab `temp/environment`, source in `bash`, echo `${SRC_URI}` and parse that for the url.
      Potentially storing in `HashMap<Filename, URI>` for later reuse.
- [ ] Cache cleanup
- [ ] Make the cache more async (e.g. fetch and serve in parallel, multiple clients served in parallel)
- [ ] Add classic Rust project claims about how *blazingly fast* and *memory safe* it is
- [ ] Add obnoxious amount of obligatory Rust project emoji like: 🦀 🚀 💥

## Build / Install

As said - this is not entirely ready yet - if you still want to test it you can compile it with `cargo`:

```
$ git clone https://github.com/xarblu/portcache.git
```

```
$ cargo build --release
```

The resulting binary will be in `target/release/portcache`


Alternatively snapshot versions will be available in my overlay `xarblu-overlay`:

```
# eselect repository enable xarblu-overlay
```

```
# emerge --sync xarblu-overlay
```

```
# emerge --ask app-portage/portcache
```

## How?

- Configure the `portcache` server as your mirror in `GENTOO_MIRRORS` in `make.conf` so `portage` will request files from `portcache`
- If `portcache` has the file then great - it can be served immediately
- If not try to fetch from a configured mirror
- If no mirror has it either look through `portage` tree to find matching `SRC_URI` and fetch that

## Why?

- There was no Gentoo-first project I could find like this.
- Closest alternative `apt-cacher-ng` caches by being a plain HTTP proxy which I'm not a fan of.  
  `portcache` directly hooks into `portage`'s mirror logic and thus can be reached via HTTP or HTTPS (when behind a reverse proxy) without touching non-distfile content.
- Nice project to learn some Rust.
- Because why not?
