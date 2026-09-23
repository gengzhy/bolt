plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "xin.cosmos.bolt"
    compileSdk = 37
    // 本机已装 36.0.0；钉住避免 AGP 自动下载
    buildToolsVersion = "36.0.0"

    defaultConfig {
        applicationId = "xin.cosmos.bolt"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            // 与 scripts/build_android_lib.bat 输出的 ABI 一致
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    signingConfigs {
        create("release") {
            val keystoreFile = rootProject.file("keystore/bolt-release.jks").takeIf { it.exists() }
                ?: file("../keystore/bolt-release.jks").takeIf { it.exists() }
                ?: file("keystore/bolt-release.jks")

            if (keystoreFile.exists()) {
                storeFile = keystoreFile
                storePassword = System.getenv("BOLT_KEYSTORE_PASSWORD")
                    ?: (project.findProperty("BOLT_KEYSTORE_PASSWORD") as String?)
                    ?: "Bolt@iangeng"
                keyAlias = System.getenv("BOLT_KEY_ALIAS")
                    ?: (project.findProperty("BOLT_KEY_ALIAS") as String?)
                    ?: "bolt"
                keyPassword = System.getenv("BOLT_KEY_PASSWORD")
                    ?: (project.findProperty("BOLT_KEY_PASSWORD") as String?)
                    ?: "Bolt@iangeng"
            } else {
                initWith(getByName("debug"))
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("release")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    base {
        archivesName.set("bolt_${defaultConfig.versionName}_universal")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    buildFeatures {
        compose = true
    }

    // jniLibs 由 scripts/build_android_lib.bat 产出（cargo-ndk）
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.kotlinx.coroutines.android)
    // SAF 目录树遍历（DocumentFile）
    implementation(libs.androidx.documentfile)
    // Compose（官方模板依赖集；编译器插件要求 Runtime 在类路径上）
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.runtime)
    implementation(libs.androidx.compose.animation)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons.core)
}

afterEvaluate {
    tasks.named("assembleDebug").configure {
        doLast {
            val apkDir = layout.buildDirectory.dir("outputs/apk/debug").get().asFile
            val ver = android.defaultConfig.versionName?.removePrefix("v") ?: "0.1.0"
            val targetName = "bolt-v${ver}-android-universal-debug.apk"
            val apk = apkDir.listFiles()?.firstOrNull { it.name.endsWith(".apk") }
            if (apk != null && apk.name != targetName) {
                apk.copyTo(File(apkDir, targetName), overwrite = true)
            }
            // 同步至 dist/debug/
            val distDebug = rootProject.projectDir.parentFile.resolve("dist/debug")
            distDebug.mkdirs()
            val finalApk = File(apkDir, targetName).takeIf { it.exists() } ?: apk
            finalApk?.copyTo(distDebug.resolve(targetName), overwrite = true)
        }
    }
    tasks.named("assembleRelease").configure {
        doLast {
            val apkDir = layout.buildDirectory.dir("outputs/apk/release").get().asFile
            val ver = android.defaultConfig.versionName?.removePrefix("v") ?: "0.1.0"
            val targetName = "bolt-v${ver}-android-universal.apk"
            val arm64Name = "bolt-v${ver}-android-arm64.apk"
            val apk = apkDir.listFiles()?.firstOrNull { it.name.endsWith(".apk") }
            if (apk != null && apk.name != targetName) {
                apk.copyTo(File(apkDir, targetName), overwrite = true)
            }
            // 同步至 dist/release/
            val distRelease = rootProject.projectDir.parentFile.resolve("dist/release")
            distRelease.mkdirs()
            val finalApk = File(apkDir, targetName).takeIf { it.exists() } ?: apk
            finalApk?.copyTo(distRelease.resolve(targetName), overwrite = true)
            finalApk?.copyTo(distRelease.resolve(arm64Name), overwrite = true)
        }
    }
}
