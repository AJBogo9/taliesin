// Register the built-ins through the public API.
var reg = window.taliEnhancers;
if (reg) {
  reg.register(taliCopyButtons);
  reg.register(function () { taliInitReaderMenu(); });
  reg.register(function () { taliInitReaderPrefs(); });
  reg.register(function () { taliInitSkipLink(); });
  reg.register(function () { taliInitKeyboard(); });
  reg.register(taliInitCategoryFilter);
  reg.register(function () { taliInitBookOutline(); });
}
