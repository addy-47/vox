# ─── JNI name-based lookups must survive R8 ───────────────────────────────────
#
# WHY THIS FILE IS NEEDED
#
# Release builds set `isMinifyEnabled = true`. R8 renames class members that it
# cannot see being called. Tauri's Android bridge, however, is NOT called from
# Java — the Rust side resolves those methods BY NAME over JNI:
#
#     env.call_method(&activity, "getId", "()I", &[])
#
# R8 cannot see those callers, so it renames the Kotlin-generated getter and the
# lookup fails at runtime with:
#
#     java.lang.NoSuchMethodError: no non-static method "Lcom/addy/Vox/MainActivity;.getId()I"
#         at com.addy.Vox.Rust.onActivityCreate(Native Method)
#
# That panic happens inside a JNI callback where unwinding is forbidden, so it
# aborts the process instead of unwinding — an unrecoverable launch crash.
#
# Note this is a `sign`-only failure mode: debug builds set
# `isMinifyEnabled = false`, so `tauri android dev` works while the release APK
# crashes on launch. Always verify release on-device.
#
# This file is separate from `proguard-rules.pro` because `build.gradle.kts`
# auto-globs `**\/*.pro`, so it is picked up without editing generated files.
# Regenerating with `tauri android init` will not delete it.

# The generated activity/webview bridge. Rust resolves `getId`, `getIntent`,
# `getLocalClassName`, and every `Rust.*` JNI method against these.
-keep class com.addy.Vox.generated.** { *; }

# Our own MainActivity — the concrete class Rust receives a jobject for.
-keep class com.addy.Vox.** { *; }

# Tauri's Java-side plugin manager and plugin implementations, likewise reached
# from Rust by name.
-keep class app.tauri.** { *; }
-keep class com.tauri.** { *; }

# Belt and braces: any class declaring `external`/native methods is a JNI entry
# point and must keep its method names.
-keepclassmembers class * {
    native <methods>;
}

# Keep the source file/line attributes so release stack traces stay readable.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
