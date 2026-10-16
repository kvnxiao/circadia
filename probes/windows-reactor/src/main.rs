//! Exercise the published Reactor controls without changing system settings.

mod curve;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_reactor::App::run_component::<ui::Probe>(())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_bindings_expose_location_and_desktop_picker_types() {
        std::hint::black_box(windows::Devices::Geolocation::Geolocator::RequestAccessAsync);
        std::hint::black_box(windows::Devices::Geolocation::Geolocator::GetGeopositionAsync);
        std::hint::black_box(windows::Storage::Pickers::FileOpenPicker::new);
        assert_ne!(
            std::mem::size_of::<windows::Win32::UI::Shell::IFileOpenDialog>(),
            0
        );
        assert_ne!(
            std::mem::size_of::<windows::Win32::UI::Shell::IInitializeWithWindow>(),
            0
        );
    }
}
