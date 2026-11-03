#!/usr/bin/env python3
"""Bind the two example workflows to a generic folder Source; never changes app settings."""
import argparse,json
from pathlib import Path,PurePosixPath
ROOT=Path(__file__).resolve().parent

def bind(workflow,folder):
    path=PurePosixPath(folder)
    if not folder or path.is_absolute() or '..' in path.parts or '\\' in folder:
        raise ValueError('Use a relative folder under the app sources root')
    result=json.loads(json.dumps(workflow))
    for n in result['nodes']:
        if n['kind']=='Source':n.update(source_type='folder',source_mode='copy',location=str(path))
    return result

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source-folder',required=True);p.add_argument('--out',default='output/workflows');a=p.parse_args();output=Path(a.out);output.mkdir(parents=True,exist_ok=True)
    for name in ['workflow-requests.json','workflow-request-to-speckit.json']:
        workflow=bind(json.loads((ROOT/name).read_text()),a.source_folder);(output/name).write_text(json.dumps(workflow,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'workflows':str(output),'source_folder':a.source_folder}))
if __name__=='__main__':main()
