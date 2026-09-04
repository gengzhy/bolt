// Top-level build file: plugin versions come from gradle/libs.versions.toml.
// Follows the official Kotlin + Compose project layout (AGP 9 built-in Kotlin,
// so the org.jetbrains.kotlin.android plugin is no longer applied).
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.compose) apply false
}
