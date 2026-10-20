// _flmextensions/talk/main.js: an extension that writes on the console when it is loaded
// and when its feature registers, and declares nothing else.
export default function (ext) {
  console.log("hello from the extension");
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      talk: {
        title: "Talk",
        register(b) {
          console.warn("registering");
        },
      },
    },
  });
}
