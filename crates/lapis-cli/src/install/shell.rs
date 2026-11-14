use std::{fs, io, path::Path};

#[derive(Clone, Copy)]
pub enum Shell {
    Bash,
    Zsh,
}
const BLOCK: &str = "# >>> Lapis CLI PATH >>>\ncase \":$PATH:\" in\n  *\":$HOME/.local/bin:\"*) ;;\n  *) export PATH=\"$HOME/.local/bin:$PATH\" ;;\nesac\n# <<< Lapis CLI PATH <<<\n";

fn read(path: &Path) -> io::Result<String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_symlink() || !m.is_file() => {
            return Err(io::Error::other(
                "Shell configuration must be a regular file, not a symlink",
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e),
        _ => {}
    }
    fs::read_to_string(path)
}
fn write(path: &Path, contents: &str) -> io::Result<()> {
    let tmp = path.with_extension(format!("lapis-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        use io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        if let Ok(m) = fs::metadata(path) {
            file.set_permissions(m.permissions())?;
        }
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    let _ = fs::remove_file(tmp);
    result
}
fn separator_block() -> String {
    format!(
        "\n{}",
        BLOCK.replace("PATH >>>", "PATH (added separator) >>>")
    )
}

pub fn setup(home: &Path, shell: Shell) -> io::Result<()> {
    let paths = match shell {
        Shell::Zsh => vec![".zshrc"],
        Shell::Bash => vec![
            ".bashrc",
            if home.join(".bash_profile").exists() {
                ".bash_profile"
            } else if home.join(".bash_login").exists() {
                ".bash_login"
            } else {
                ".profile"
            },
        ],
    };
    for name in paths {
        let path = home.join(name);
        let mut text = read(&path)?;
        if text.contains(BLOCK) || text.contains(&separator_block()) {
            continue;
        }
        if text.contains("# >>> Lapis CLI PATH") {
            return Err(io::Error::other(
                "A modified Lapis PATH block already exists",
            ));
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push_str(&separator_block());
        } else {
            text.push_str(BLOCK);
        }
        write(&path, &text)?;
    }
    Ok(())
}
pub fn remove(home: &Path) -> io::Result<()> {
    for name in [
        ".bashrc",
        ".bash_profile",
        ".bash_login",
        ".profile",
        ".zshrc",
    ] {
        let path = home.join(name);
        let text = read(&path)?;
        let next = text.replace(&separator_block(), "").replace(BLOCK, "");
        if next != text {
            write(&path, &next)?;
        }
    }
    Ok(())
}
