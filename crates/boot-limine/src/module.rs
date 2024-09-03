use limine::ModuleRequest;

//

pub fn modules() -> impl Iterator<Item = Module> {
    static MOD_REQ: ModuleRequest = ModuleRequest::new(0);

    let all_modules = MOD_REQ
        .get_response()
        .get()
        .map_or(&[][..], |resp| resp.modules());

    all_modules.iter().filter_map(|file| {
        Some(Module {
            addr: file.base.as_ptr()? as usize,
            size: file.length as usize,
            path: file.path.to_str().and_then(|c| c.to_str().ok()),
            cmdline: file.cmdline.to_str().and_then(|c| c.to_str().ok()),
        })
    })
}

//

#[derive(Debug, Clone, Copy)]
pub struct Module {
    pub addr: usize,
    pub size: usize,
    pub path: Option<&'static str>,
    pub cmdline: Option<&'static str>,
}
