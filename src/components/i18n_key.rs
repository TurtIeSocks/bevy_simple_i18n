use super::define_i18n_text_component;

define_i18n_text_component!(
    /// Translation-key driver with no built-in render target (see
    /// [`register_i18n_writer`](crate::prelude::I18nComponentRegistration::register_i18n_writer)).
    ///
    /// Pair it with any third-party text component and a closure that writes
    /// the translated string — the escape hatch when an `I18nTarget` impl is
    /// impossible (orphan rule: foreign trait + foreign type).
    I18nKey, no_target
);
