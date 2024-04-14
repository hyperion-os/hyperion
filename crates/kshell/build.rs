use std::{
    env,
    fs::{read_dir, File},
    io::Write,
    path::Path,
};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let asset_rs_file = Path::new(&out_dir).join("asset.rs");

    let mut asset_rs_file = File::create(asset_rs_file).unwrap();

    writeln!(asset_rs_file, "pub static ASSETS: &[(&str, &[u8])] = &[").unwrap();

    let from = "./../../asset";
    travel(from.as_ref(), &mut |path| {
        let os_canonical = if path.is_symlink() {
            path.read_link().unwrap()
        } else {
            path.canonicalize().unwrap()
        };

        if !os_canonical.exists() {
            return;
        }

        let path = Path::new("/").join(path.strip_prefix(from).unwrap());

        println!("cargo:rerun-if-changed={os_canonical:?}");
        writeln!(
            asset_rs_file,
            "    ({path:?}, include_bytes!({os_canonical:?})),",
        )
        .unwrap();
    });

    writeln!(asset_rs_file, "];").unwrap();
    println!("cargo:rerun-if-changed={from}");

    // panic!("{:?}", current_dir().unwrap());
    // panic!("{out_dir}");
}

fn travel(path: &Path, f: &mut impl FnMut(&Path)) {
    for ent in read_dir(path).unwrap() {
        let ent = ent.unwrap();

        let ty = ent.file_type().unwrap();
        let p = ent.path();

        if ty.is_file() || ty.is_symlink() {
            f(&p);
        } else if ty.is_dir() {
            travel(&p, f);
        }
    }
}
