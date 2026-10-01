- **`axios` 1.20.0 in the web interface.** The bundled 1.18.1 fell under
  twelve advisories fixed in 1.20.0. Most concern Node's HTTP adapters,
  which a browser bundle never loads; the rest are prototype-pollution
  gadgets and header injection through inherited properties, reachable only
  together with a pollution bug elsewhere in the page. `frontend/dist` is
  rebuilt with it. The build tooling is refreshed within its ranges as well
  (`postcss` 8.5.28, `browserslist` 4.29.3, `nanoid` 3.3.19). `vite` 4 and
  its `esbuild` stay: their advisories concern the development server, and
  the fix is the major upgrade to `vite` 8, which deserves a change of its
  own.
