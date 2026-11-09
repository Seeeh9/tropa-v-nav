#!/bin/sh
# Builds immersive.dex from Immersive.java (needs a JDK and the Android SDK at $ANDROID_HOME).
# The dex is committed; run this only when the Java changes.
set -e
cd "$(dirname "$0")"
sdk=${ANDROID_HOME:-$ANDROID_SDK_ROOT}
jar=$(ls -d "$sdk"/platforms/android-3[4-9]/android.jar | tail -1)
d8=$(ls -d "$sdk"/build-tools/*/ | tail -1)d8
[ -x "$d8" ] || d8="$d8.bat"
out=$(mktemp -d)
javac -source 8 -target 8 -bootclasspath "$jar" -d "$out" tropa/Immersive.java
"$d8" --min-api 26 --lib "$jar" --output "$out" "$out"/tropa/Immersive.class
cp "$out/classes.dex" ../immersive.dex
