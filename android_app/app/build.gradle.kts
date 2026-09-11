plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.lt.transfer"
    compileSdk = 37
    // 本机已装 36.0.0；钉住避免 AGP 自动下载
    buildToolsVersion = "36.0.0"

    defaultConfig {
        applicationId = "com.lt.transfer"
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
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
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
