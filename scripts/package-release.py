#!/usr/bin/env python3
"""Stage the matching Rust/helper pair, notices, and optional relink bundle without installing it."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def copy_licenses(source,destination):
    destination.mkdir(parents=True,exist_ok=True)
    files = [file for file in source.iterdir() if file.is_file() and file.name.upper().startswith(("LICEN","COPYING","COPYRIGHT","NOTICE"))]
    for file in files:
        shutil.copy2(file,destination/file.name)
    return files


def rust_notices(destination,target):
    metadata = json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version=1","--filter-platform="+target],cwd=ROOT))
    nodes = {node["id"]:node for node in metadata["resolve"]["nodes"]}
    pending,used = [metadata["resolve"]["root"]],set()
    while pending:
        node = pending.pop()
        if node in used: continue
        used.add(node)
        pending.extend(dep["pkg"] for dep in nodes[node]["deps"] if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
    rows = []
    for package in metadata["packages"]:
        if package["id"] not in used: continue
        name = package["name"]+"-"+package["version"]
        folder = Path(package["manifest_path"]).parent
        output = destination/name
        files = copy_licenses(folder,output)
        extra = ROOT/"third_party/rust-extra"/name
        if extra.exists():
            shutil.copytree(extra,output,dirs_exist_ok=True)
            files.extend(file for file in extra.iterdir() if file.is_file())
        if not files:
            raise RuntimeError(f"Missing upstream notice for {name}; supply its pinned license under third_party/rust-extra")
        if package["name"] == "whisper-rs-sys":
            for file in folder.rglob("LICENSE*"):
                if file.is_file():
                    relative = file.relative_to(folder)
                    (output/relative.parent).mkdir(parents=True,exist_ok=True)
                    shutil.copy2(file,output/relative)
        rows.append({"name":package["name"],"version":package["version"],"license":package["license"],
                     "repository":package["repository"],"authors":package["authors"]})
    return rows


def javascript_packages(metafile):
    inputs = json.loads(metafile.read_text())["inputs"]
    names = set()
    for path in inputs:
        if "node_modules/" not in path: continue
        parts = path.split("node_modules/",1)[1].split("/")
        names.add("/".join(parts[:2]) if parts[0].startswith("@") else parts[0])
    if "@anthropic-ai/claude-agent-sdk" not in names:
        raise RuntimeError("Helper bundle does not contain the official SDK")
    return sorted(names)


def notices(destination,target,metafile):
    destination.mkdir(parents=True)
    shutil.copytree(ROOT/"third_party/bun-1.3.10",destination/"bun-1.3.10")
    rows = []
    for name in javascript_packages(metafile):
        package = ROOT/"helpers/claude-controller/node_modules"/name
        info = json.loads((package/"package.json").read_text())
        if not copy_licenses(package,destination/"javascript"/name.replace("/","_")):
            raise RuntimeError("Missing JavaScript notice: "+name)
        rows.append({"name":name,"version":info["version"],"license":info.get("license")})
    (destination/"inventory.json").write_text(json.dumps({"bun":"1.3.10","javascript":rows,
        "rustBuildInputs":rust_notices(destination/"rust",target)},indent=2)+"\n")
    shutil.copy2(ROOT/"third_party/README.md",destination/"README.md")


def check_binary_target(path,target):
    with path.open("rb") as stream:
        header = stream.read(64)
        if "linux" in target:
            valid = header[:4] == b"\x7fELF" and header[4] == 2 and int.from_bytes(header[18:20],"little") == 62
        elif "darwin" in target:
            cpu = 0x100000c if target.startswith("aarch64") else 0x1000007
            valid = header[:4] == b"\xcf\xfa\xed\xfe" and int.from_bytes(header[4:8],"little") == cpu
        elif "windows" in target:
            stream.seek(int.from_bytes(header[60:64],"little"))
            pe = stream.read(6)
            valid = header[:2] == b"MZ" and pe[:4] == b"PE\0\0" and pe[4:] == b"\x64\x86"
        else: valid = False
    if not valid: raise RuntimeError(f"{path} does not contain the selected {target} binary")


def package(binary,helper,target,output):
    if output.exists() and any(output.iterdir()):
        raise RuntimeError("Choose a fresh empty package folder: "+str(output))
    build = json.loads((helper.parent/"claude-controller-build.json").read_text())
    if build["rustTarget"] != target or build["sdk"] != "0.3.322" or build["bun"] != "1.3.10":
        raise RuntimeError("Helper does not match the selected pinned release target")
    check_binary_target(binary,target)
    check_binary_target(helper,target)
    output.mkdir(parents=True,exist_ok=True)
    extension = ".exe" if "windows" in target else ""
    shutil.copy2(binary,output/("carlos"+extension))
    shutil.copy2(helper,output/("carlos-claude-controller"+extension))
    shutil.copy2(ROOT/"LICENSE",output/"LICENSE")
    shutil.copy2(ROOT/"README.md",output/"README.md")
    shutil.copy2(helper.parent/"claude-controller-build.json",output/"claude-controller-build.json")
    shutil.copytree(helper.parent/"relink",output/"relink",dirs_exist_ok=True)
    shutil.copy2(ROOT/"helpers/claude-controller/RELINK.md",output/"relink/README.md")
    notices(output/"third-party-notices",target,helper.parent/"controller-metafile.json")
    files = sorted(file for file in output.rglob("*") if file.is_file())
    (output/"SHA256SUMS").write_text("".join(hashlib.sha256(file.read_bytes()).hexdigest()+"  "+file.relative_to(output).as_posix()+"\n" for file in files))
    return output


def archive(folder,path):
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.suffix == ".zip":
        with zipfile.ZipFile(path,"w",compression=zipfile.ZIP_DEFLATED) as output:
            for file in sorted(folder.rglob("*")):
                if file.is_file(): output.write(file,file.relative_to(folder))
    else:
        with tarfile.open(path,"w:gz") as output:
            for file in sorted(folder.iterdir()): output.add(file,arcname=file.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary",type=Path,required=True)
    parser.add_argument("--helper",type=Path,required=True)
    parser.add_argument("--target",required=True)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--archive",type=Path)
    options = parser.parse_args()
    folder = package(options.binary.resolve(),options.helper.resolve(),options.target,options.output.resolve())
    if options.archive: archive(folder,options.archive.resolve())
    print("Staged release pair:",folder)


if __name__ == "__main__":
    main()
