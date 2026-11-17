The icon uses the app’s blue prompt and green cursor on its Mocha background.
The SVG is the editable source; the committed PNG lets Xcode build without an image tool.
Regenerate it from the repository root with:

    rsvg-convert -w 1024 -h 1024 apps/ios/Design/AppIcon.svg -o apps/ios/Resources/Assets.xcassets/AppIcon.appiconset/AppIcon.png

The PNG has an opaque background. iOS supplies the icon’s outer corner mask.
