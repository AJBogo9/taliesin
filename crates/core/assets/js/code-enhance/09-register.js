// Register the built-ins through the public API.
var reg = window.taliEnhancers;
if (reg) {
  reg.register(taliCopyButtons);
  reg.register(function () { taliInitSkipLink(); });
  reg.register(function () { taliInitKeyboard(); });
}
