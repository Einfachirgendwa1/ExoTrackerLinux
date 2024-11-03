#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub use linux::*;

#[cfg(windows)]
mod windows {
    use crate::model::{Model, Snapshot, Status};
    use crate::resolve_multilevel_pointer;
    use nalgebra::Vector3;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};
    use win_mem::process::Process;
    use win_mem::utils::WinResult;

    pub fn begin(model_arc: Arc<Mutex<Model>>) {
        loop {
            {
                model_arc.lock().unwrap().status = Status::NoGame;
            }

            let process = loop {
                if let Ok(process) = Process::find("EXO ONE.exe") {
                    break process;
                }
                thread::sleep(Duration::from_millis(1000));
            };

            {
                model_arc.lock().unwrap().status = Status::Menu;
            } //desktop->menu

            let mut in_menu = true;
            while let Ok(unity_player) = process.find_module("UnityPlayer.dll") {
                while read_and_log(&model_arc, &process, unity_player.address()).is_ok() {
                    in_menu = false;
                    thread::sleep(Duration::from_millis(100));
                }

                if !in_menu {
                    {
                        model_arc.lock().unwrap().status = Status::Menu;
                    } //ingame -> menu
                    in_menu = true;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    fn read_and_log(
        model_arc: &Arc<Mutex<Model>>,
        process: &Process,
        unity_player: usize,
    ) -> WinResult<()> {
        let address = resolve_multilevel_pointer(
            process,
            unity_player + 0x01A03D00,  //0x0156C900
            &[0x58, 0x158, 0x28, 0xA0], //0x3F8, 0x1A8, 0x28, 0xA0
        )?;

        //let position = process.read_mem::<Vector3<f32>>(address + 0x00)?;
        let velocity = process.read_mem::<Vector3<f32>>(address + 0x30)?;

        let now = Instant::now();
        let mut model = model_arc.lock().unwrap();
        model.status = Status::Active;
        while let Some(front) = model.snapshots.front() {
            if (now - front.timestamp).as_millis() < 60000 {
                break;
            }
            model.snapshots.pop_front().expect("impossible");
        }
        model.snapshots.push_back(Snapshot {
            velocity,
            timestamp: now,
        });

        Ok(())
    }
}

#[cfg(not(windows))]
mod linux {
    use std::{
        fs::{read_dir, File},
        io::Read,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    const NEEDLE: &'static [u8] = b"Name:\tExo One\n";

    use crate::model::{Model, Status};

    struct ExoOneProcess {
        mem_file: File,
        maps_file: File,
    }

    impl ExoOneProcess {
        fn find() -> Option<ExoOneProcess> {
            read_dir("/proc")
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .map(|entry| move |path: &'static str| File::open(entry.join(path)).unwrap())
                .filter(|open| {
                    let mut buf = [0; NEEDLE.len()];
                    open("status").read(&mut buf);
                    buf == NEEDLE
                })
                .map(|open| ExoOneProcess {
                    mem_file: open("mem"),
                    maps_file: open("maps"),
                })
                .next()
        }

        fn find_unity_player(&mut self) -> usize {
            let mut maps = String::new();
            self.maps_file.read_to_string(&mut maps);

            maps.split('\n')
                .filter(|line| !line.is_empty())
                .map(str::split_whitespace)
                .find_map(|mut line| {
                    let address = line.next().unwrap().split_once('-').unwrap().0;

                    line.skip(4)
                        .collect::<String>()
                        .ends_with("/UnityPlayer.dll")
                        .then(|| address)
                })
                .and_then(|unity_player| usize::from_str_radix(unity_player, 16).ok())
                .unwrap()
        }
    }

    pub fn begin(model_arc: Arc<Mutex<Model>>) {
        loop {
            model_arc.lock().unwrap().status = Status::NoGame;

            let process = {
                loop {
                    if let Some(process) = ExoOneProcess::find() {
                        break process;
                    }
                    thread::sleep(Duration::from_millis(1000));
                }
            };

            model_arc.lock().unwrap().status = Status::Menu;
        }
    }
}
