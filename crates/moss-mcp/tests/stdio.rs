use moss_mcp::{Request, Response};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
};

#[test]
fn stdio_mcp_client_routes_to_local_socket() {
    let root = PathBuf::from("/tmp").join(format!("mcp-test-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(root.join("mcp")).unwrap();
    fs::write(root.join("mcp/token"), "test-secret").unwrap();
    let listener = UnixListener::bind(root.join("mcp/service.sock")).unwrap();
    let backend = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        let request: Request = serde_json::from_str(&line).unwrap();
        assert_eq!(request.token, "test-secret");
        assert_eq!(request.actor_name, "Test Agent");
        assert_eq!(request.method, "read_page");
        assert_eq!(request.workspace_id, "workspace");
        assert_eq!(request.args["page_id"], "page");
        writeln!(
            stream,
            "{}",
            serde_json::to_string(&Response::ok(json!({"page":"hello"}))).unwrap()
        )
        .unwrap();
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_moss-mcp"))
        .env("MOSS_DATA_DIR", &root)
        .env("MOSS_AGENT_NAME", "Test Agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let exchange = |input: &mut std::process::ChildStdin,
                    output: &mut BufReader<std::process::ChildStdout>,
                    value: Value|
     -> Value {
        writeln!(input, "{value}").unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let init = exchange(
        &mut input,
        &mut output,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
    );
    assert_eq!(init["id"], 1);
    assert!(init.get("result").is_some(), "{init}");
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let list = exchange(
        &mut input,
        &mut output,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    );
    for name in [
        "read_page",
        "create_page",
        "rename_page",
        "list_trash",
        "trash_page",
        "restore_page",
    ] {
        assert!(
            list["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == name)
        );
    }
    let result = exchange(
        &mut input,
        &mut output,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read_page","arguments":{"workspace_id":"workspace","page_id":"page"}}}),
    );
    assert_eq!(result["id"], 3);
    assert!(result.to_string().contains("hello"));
    assert!(!result.to_string().contains("test-secret"));
    drop(input);
    let _ = child.wait();
    backend.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
