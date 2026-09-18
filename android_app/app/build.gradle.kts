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

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("debug")
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
            val targetName = "bolt_${android.defaultConfig.versionName}_universal-debug.apk"
            val apk = apkDir.listFiles()?.firstOrNull { it.name.endsWith(".apk") && it.name != "bolt.apk" }
            if (apk != null && apk.name != targetName) {
                apk.copyTo(File(apkDir, targetName), overwrite = true)
            }
            apk?.copyTo(File(apkDir, "bolt.apk"), overwrite = true)
            // 同步至 dist/android/debug/
            val distDebug = rootProject.projectDir.parentFile.resolve("dist/android/debug")
            distDebug.mkdirs()
            val finalApk = File(apkDir, targetName).takeIf { it.exists() } ?: apk
            finalApk?.copyTo(distDebug.resolve(targetName), overwrite = true)
        }
    }
    tasks.named("assembleRelease").configure {
        doLast {
            val apkDir = layout.buildDirectory.dir("outputs/apk/release").get().asFile
            val targetName = "bolt_${android.defaultConfig.versionName}_universal.apk"
            val apk = apkDir.listFiles()?.firstOrNull { it.name.endsWith(".apk") && it.name != "bolt.apk" && it.name != "bolt-release.apk" }
            if (apk != null && apk.name != targetName) {
                apk.copyTo(File(apkDir, targetName), overwrite = true)
            }
            apk?.copyTo(File(apkDir, "bolt.apk"), overwrite = true)
            apk?.copyTo(File(apkDir, "bolt-release.apk"), overwrite = true)
            // 同步至 dist/android/release/
            val distRelease = rootProject.projectDir.parentFile.resolve("dist/android/release")
            distRelease.mkdirs()
            val finalApk = File(apkDir, targetName).takeIf { it.exists() } ?: apk
            finalApk?.copyTo(distRelease.resolve(targetName), overwrite = true)
        }
    }
}
