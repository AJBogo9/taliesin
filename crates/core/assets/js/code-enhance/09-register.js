// Register the built-ins through the public API. Link-preview sets itself up
// once (document-level), so it ignores `root`.
var reg = window.taliEnhancers;
if (reg) {
  reg.register(taliCopyButtons);
  reg.register(function () { taliInitLinkPreview(); });
  reg.register(function () { taliInitReaderMenu(); });
  reg.register(function () { taliInitReaderPrefs(); });
  reg.register(function () { taliInitReadingProgress(); });
  reg.register(taliInitAnchorLinks);
  reg.register(function () { taliInitSkipLink(); });
  reg.register(function () { taliInitKeyboard(); });
  reg.register(taliInitCategoryFilter);
  reg.register(function () { taliInitBookOutline(); });
  reg.register(function () { taliInitCodeVisibility(); });
}
