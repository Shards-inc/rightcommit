#!/usr/bin/env python3
"""Validate already identified catalog Requests; no credentials, network or AI routing."""
import argparse,json,re
from pathlib import Path

def build(config,tickets):
    if config.get('schema')!='rightcommit.servicenow.catalog.v1':
        raise ValueError('Invalid catalog configuration schema')
    namespace=config.get('namespace','')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}',namespace) or tickets.get('namespace')!=namespace:
        raise ValueError('Stable namespace differs between configuration and requests')
    rows=tickets.get('tickets',[])
    if not isinstance(rows,list) or not 1<=len(rows)<=100:
        raise ValueError('Provide 1..100 Requests')
    seen=set();plans=[]
    for row in rows:
        request=row['request_id'];text=row['description'].strip();route=row.get('catalog_route')
        if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}',request) or request in seen:
            raise ValueError('Invalid or duplicate request_id')
        if not text or len(text)>12000 or route not in config.get('catalog',{}) or not isinstance(row.get('fields',{}),dict):
            raise ValueError('Invalid description, catalog_route or fields')
        seen.add(request)
        plans.append({'request_id':request,'category':'Request','action':'order_catalog','description':text,
                      'short_description':text[:160],'answer':'Request catalogo da configurazione file',
                      'status':'request_prepared','catalog_route':route,'request_fields':row.get('fields',{})})
    return {'schema':'rightcommit.itsm.batch.v1','namespace':namespace,'tickets':plans}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--config',default='config/catalog-routing.json');p.add_argument('--tickets',default='requests.json');p.add_argument('--out',default='output/request-plans.json');a=p.parse_args()
    result=build(json.loads(Path(a.config).read_text()),json.loads(Path(a.tickets).read_text()))
    output=Path(a.out);output.parent.mkdir(parents=True,exist_ok=True)
    for plan in result['tickets']:
        path=output.parent/'plans'/(plan['request_id']+'.json');path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
    output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'plans':len(result['tickets']),'output':str(output)}))
if __name__=='__main__':main()
