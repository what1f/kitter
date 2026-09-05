# macOS app icon

`Kitter.icon` is the editable Icon Composer source. Its artwork preserves the
existing Kitter illustration from `../app-icon.png`.

`app-icon.png` is the committed 1024×1024 RGBA packaging asset. Icon Composer
renders the macOS rounded mask and material; the export script scales that result
to an 824×824 footprint centered in a transparent canvas (100px inset per side).
Do not use the full-bleed cross-platform PNG to generate the macOS ICNS.

Regenerate on macOS with Icon Composer 2.x (design generation 27) installed:

```sh
just macos-icon
```

Set `ICON_COMPOSER_TOOL` if `ictool` is installed elsewhere. The release build
uses the committed PNG, so CI and contributors do not need Icon Composer.
`just app` generates all ten standard ICNS representations and embeds
`logo.icns` via `CFBundleIconFile` before signing the bundle.

This uses the static ICNS path with the app's macOS 12 deployment target.
The icon was verified in the installer, Launchpad, and Dock on macOS 15.6.1;
macOS 27 was checked through the system icon API. Other versions have not
been individually tested. The `.icon`
source itself is not a compiled app resource: shipping dynamic Liquid Glass
appearances requires a future Xcode asset-compilation step, an asset catalog
icon name in the bundle, and the ICNS fallback for older systems. Do not embed
an uncompiled `.icon` directory and assume that enables dynamic appearances.

References:
- https://developer.apple.com/design/human-interface-guidelines/app-icons
- https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer
