// Kova iOS — thin Swift wrapper around Rust egui app
// Unlicense — cochranblock.org

import UIKit

// Bridge to Rust static library
@_silgen_name("kova_ios_main")
func kova_ios_main()

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Launch the Rust egui app on a background thread
        // (eframe::run_native blocks, so it needs its own thread)
        DispatchQueue.global(qos: .userInitiated).async {
            kova_ios_main()
        }
        return true
    }
}
