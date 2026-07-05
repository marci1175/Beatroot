use rodio::cpal::traits::HostTrait;

fn list_output_devices() -> Vec<rodio::cpal::Device> {
    let host = rodio::cpal::default_host();

    match host.output_devices() {
        Ok(devices) => devices.collect(),
        Err(_) => Vec::new(),
    }
}
