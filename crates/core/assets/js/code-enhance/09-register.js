// Register the built-ins through the public API. Lightbox / link-preview set
// themselves up once (document-level), so they ignore `root`.
var reg = window.taliEnhancers;
if (reg) {
  reg.register(taliCopyButtons);
  reg.register(function () { taliInitLightbox(); });
  reg.register(function () { taliInitLinkPreview(); });
  reg.register(function () { taliInitReaderMenu(); });
  reg.register(function () { taliInitReaderPrefs(); });
  reg.register(function () { taliInitReadingProgress(); });
  reg.register(taliInitAnchorLinks);
  reg.register(function () { taliInitFocusMode(); });
  reg.register(function () { taliInitSkipLink(); });
  reg.register(function () { taliInitKeyboard(); });
  reg.register(taliInitCategoryFilter);
  reg.register(taliInitCiteBox);
  reg.register(function () { taliInitBookOutline(); });
}
