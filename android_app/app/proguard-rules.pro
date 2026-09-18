# Bolt Android 混淆规则
-keep class xin.cosmos.bolt.ffi.Native { *; }
-keepclasseswithmembernames class * {
    native <methods>;
}
-keep class xin.cosmos.bolt.engine.** { *; }
-dontwarn xin.cosmos.bolt.**
