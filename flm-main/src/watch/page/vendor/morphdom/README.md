# Vendored: `morphdom`

The page script of the live preview brings a changed part of the page up to date with
`morphdom`, which changes an element into another while keeping every element that is the
same in both.

- **Package**: `morphdom` on the npm registry, version **2.7.8** (license MIT, in `LICENSE`
  beside this file, copied from the package).
- **File**: `morphdom-umd.js` is the package's `dist/morphdom-umd.js`, unmodified. It is the
  form of the package that a script can carry inside its own function: given a local
  `module` object, it hands its function over through it and defines no global name.
- **Origin**: `https://registry.npmjs.org/morphdom/-/morphdom-2.7.8.tgz`, whose SHA-1 the
  registry states as `9a81aae144136702d58cfae8226470bfd64fe74c`.

To refresh it, from this folder (replace the version in the three places):

```sh
curl -sSL https://registry.npmjs.org/morphdom/-/morphdom-2.7.8.tgz | tar xz --strip-components=1 package/dist/morphdom-umd.js package/LICENSE && mv dist/morphdom-umd.js . && rmdir dist
```
