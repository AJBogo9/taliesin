// Register the built-ins through the public API. Lightbox / link-preview set
// themselves up once (document-level), so they ignore `root`.
window.qmdEnhancers.register(qmdCopyButtons);
window.qmdEnhancers.register(function () { qmdInitLightbox(); });
window.qmdEnhancers.register(function () { qmdInitLinkPreview(); });
window.qmdEnhancers.register(function () { qmdInitReaderMenu(); });
window.qmdEnhancers.register(function () { qmdInitReaderPrefs(); });
window.qmdEnhancers.register(function () { qmdInitReadingProgress(); });
window.qmdEnhancers.register(function () { qmdInitHighlights(); });
window.qmdEnhancers.register(function () { qmdInitHighlightIndex(); });
window.qmdEnhancers.register(function () { qmdInitBookmarks(); });
window.qmdEnhancers.register(qmdInitAnchorLinks);
window.qmdEnhancers.register(function () { qmdInitFocusMode(); });
window.qmdEnhancers.register(function () { qmdInitReadAloud(); });
window.qmdEnhancers.register(function () { qmdInitSkipLink(); });
window.qmdEnhancers.register(function () { qmdInitKeyboard(); });
window.qmdEnhancers.register(qmdInitCategoryFilter);

