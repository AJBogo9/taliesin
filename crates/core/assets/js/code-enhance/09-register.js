// Register the built-ins through the public API. Lightbox / link-preview set
// themselves up once (document-level), so they ignore `root`.
window.taliEnhancers.register(taliCopyButtons);
window.taliEnhancers.register(function () { taliInitLightbox(); });
window.taliEnhancers.register(function () { taliInitLinkPreview(); });
window.taliEnhancers.register(function () { taliInitReaderMenu(); });
window.taliEnhancers.register(function () { taliInitReaderPrefs(); });
window.taliEnhancers.register(function () { taliInitReadingProgress(); });
window.taliEnhancers.register(taliInitAnchorLinks);
window.taliEnhancers.register(function () { taliInitFocusMode(); });
window.taliEnhancers.register(function () { taliInitSkipLink(); });
window.taliEnhancers.register(function () { taliInitKeyboard(); });
window.taliEnhancers.register(taliInitCategoryFilter);

