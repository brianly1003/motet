//! Exercise subprocess settings without recording or contacting a model server.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn local_host_and_empty_vad_survive_conflicting_shell_settings() {
    let dir = std::env::temp_dir().join(format!(
        "motetto-process-{}-{}",
        std::process::id(),
        std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    for (name, script) in [
        ("ffmpeg", "#!/bin/sh\nexit 99\n"),
        (
            "whisper-cli",
            "#!/bin/sh\nprintf '%s\n' \"$@\" > whisper-args\nprintf '0,1000,\" Say \"\"go\"\"\"\n' > call.csv\n",
        ),
        (
            "ollama",
            "#!/bin/sh\n[ \"$OLLAMA_HOST\" = http://127.0.0.1:11434 ] || exit 22\nprintf '%s\n' \"$1\" >> ollama-calls\nif [ \"$1\" = list ]; then\n  printf 'NAME ID SIZE\ntest:latest abc 1GB\n'\nelse\n  cat >/dev/null\n  printf '[00:00] Say \"go\"\n'\nfi\n",
        ),
    ] {
        let path = dir.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut wav = hound::WavWriter::create(
        dir.join("call.wav"),
        hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    wav.write_sample(0_i16).unwrap();
    wav.finalize().unwrap();
    for (vad, expected) in [
        ("WHISPER_VAD_MODEL=\n", None),
        ("", None),
        ("WHISPER_VAD_MODEL=enabled.bin\n", Some("enabled.bin")),
    ] {
        fs::write(
            dir.join(".env"),
            format!("WHISPER_MODEL=fake.bin\n{vad}OLLAMA_MODEL=test\nFIX_MODEL=test\nSUMMARY_CHUNK_CHARS=24000\n"),
        ).unwrap();
        fs::write(dir.join("ollama-calls"), "").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_motetto"));
        command
            .args(["run", "--file", "call.wav"])
            .current_dir(&dir)
            .env("HOME", &dir)
            .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
            .env("OLLAMA_HOST", "http://192.0.2.1:11434")
            .env_remove("WHISPER_VAD_MODEL");
        if vad == "WHISPER_VAD_MODEL=\n" {
            command.env("WHISPER_VAD_MODEL", "stale-model.bin");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(dir.join("call.txt")).unwrap(),
            "[00:00] Say \"go\"\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("call.fixed.txt")).unwrap(),
            "[00:00] Say \"go\"\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("ollama-calls")).unwrap(),
            "list\nrun\nlist\nrun\n"
        );
        let args = fs::read_to_string(dir.join("whisper-args")).unwrap();
        match expected {
            Some(model) => assert!(args.contains(&format!("--vad\n-vm\n{model}\n"))),
            None => assert!(!args.lines().any(|a| a == "--vad" || a == "-vm")),
        }
    }
    fs::remove_dir_all(&dir).unwrap();
}
