# One-shot transport helper, streamed through SSH/WSL. No installed service.
import sys,json,os,pathlib,tempfile,shutil,hashlib,platform,datetime,configparser,fnmatch
req=json.load(sys.stdin)
def guard(path):
    resolved=path.resolve()
    for destination in req.get('_blocked_mounts',[]):
        mount=pathlib.Path(destination).resolve()
        for candidate in (resolved,resolved.parent):
            if candidate==mount or mount in candidate.parents or candidate in mount.parents:
                raise ValueError('Path overlaps a shared container mount; map its host folder once or use an unmounted provider directory')
def recover(path):
    journal=path.parent/('.hk-transaction-'+hashlib.sha256(path.name.encode()).hexdigest()+'.json')
    if not journal.exists(): return
    info=json.loads(journal.read_text());old=info['old']
    if '/' in old or '\\' in old or not old.startswith('.hk-old-'): raise ValueError('Unsafe transaction backup')
    backup=path.parent/old
    if not path.exists() and backup.exists(): backup.rename(path)
    journal.unlink()
def folder(path):
    recover(path)
    if path.is_symlink(): raise ValueError('Symlink needs review')
    if not path.exists(): return None
    files={};modes={};directories=[];total=0
    for base,dirs,names in os.walk(path,followlinks=False):
        for name in dirs+names:
            if (pathlib.Path(base)/name).is_symlink(): raise ValueError('Symlink needs review')
        directories.extend((pathlib.Path(base)/name).relative_to(path).as_posix() for name in dirs)
        for name in sorted(names):
            item=pathlib.Path(base)/name;total+=item.stat().st_size
            if total>64*1024*1024 or len(files)>=10000: raise ValueError('Skill exceeds size limit')
            relative=item.relative_to(path).as_posix();files[relative]=list(item.read_bytes());modes[relative]=(item.stat().st_mode & 0o777) if os.name!='nt' else 0o644
    files=dict(sorted(files.items()));modes=dict(sorted(modes.items()));directories.sort()
    digest=hashlib.sha256(json.dumps([files,modes,directories],ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
    return {'files':files,'modes':modes,'directories':directories,'fingerprint':digest}
def signature(snapshot): return snapshot['fingerprint'] if snapshot else None
def safe_path(path):
    for part in [path]+list(path.parents):
        if part.is_symlink(): raise ValueError('Symlink paths are not traversed')
    guard(path)
    return path.resolve(strict=True)
def metadata_entries(root):
    entries=[];truncated=False
    with os.scandir(root) as stream:
        for index,entry in enumerate(stream):
            if index>=10000: truncated=True;break
            if not entry.is_symlink(): entries.append(entry)
    return sorted(entries,key=lambda e:(not e.is_dir(follow_symlinks=False),e.name)),truncated
def contains_skill_files(root):
    try:
        with os.scandir(root) as entries:
            for index,entry in enumerate(entries):
                if index>=10000: break
                if not entry.is_dir(follow_symlinks=False): continue
                for name in ('SKILL.md','SKILL.md.disabled'):
                    path=pathlib.Path(entry.path)/name
                    if not path.is_symlink() and path.is_file(): return True
    except OSError: pass
    return False
def component_paths(root,pattern):
    if not any(c in pattern for c in '*?['): return [root/pattern]
    base=root
    for part in pattern.split('/'):
        if any(c in part for c in '*?['): break
        base=base/part
    try: safe_path(base)
    except (OSError,ValueError): return []
    result=[];pending=[(base,0)];visited=0
    while pending and visited<1000:
        path,depth=pending.pop();visited+=1
        if path.relative_to(root).match(pattern): result.append(path)
        if path.is_dir() and depth<12:
            try: entries,_=metadata_entries(path)
            except (OSError,ValueError): continue
            pending.extend((pathlib.Path(e.path),depth+1) for e in entries)
            pending=pending[:1000-visited]
    return result
try:
    action=req['action']
    if action=='managed_scan':
        result=[];seen=set()
        for source in req['sources']:
            root=pathlib.Path(source['root'])
            if not root.exists(): continue
            root=safe_path(root)
            for candidate in component_paths(root,source['pattern']) if source['pattern'] else [root]:
                if not candidate.exists(): continue
                candidate=safe_path(candidate)
                if candidate!=root and root not in candidate.parents: raise ValueError('File outside inventory root')
                paths=[candidate]
                if candidate.is_dir():
                    paths=[]
                    for base,dirs,names in os.walk(candidate,followlinks=False):
                        dirs[:]=[d for d in dirs if d not in ('.git','node_modules','target','.venv','__pycache__') and not (pathlib.Path(base)/d).is_symlink()]
                        if len(pathlib.Path(base).relative_to(candidate).parts)>=16: dirs[:]=[]
                        paths.extend(pathlib.Path(base)/name for name in names)
                        if len(paths)>10000: raise ValueError('Inventory exceeds file limit')
                for path in paths:
                    key=(source.get('group',''),str(path))
                    if path.is_symlink() or not path.is_file() or key in seen: continue
                    if len(seen)>=10000: raise ValueError('Inventory exceeds file limit')
                    seen.add(key);content=None;fingerprint=None;error=None
                    try:
                        safe_path(path)
                        if path.stat().st_size>4*1024*1024: raise ValueError('File exceeds text size limit')
                        data=path.read_bytes()
                        if b'\x00' in data: raise ValueError('Binary file')
                        content=data.decode('utf-8');fingerprint=hashlib.sha256(data).hexdigest()
                    except (ValueError,OSError) as exc: error=str(exc)
                    result.append({'group':source.get('group',''),'path':str(path),'root':str(root),'relative':path.relative_to(root).as_posix(),'category':source['category'],'content':content,'fingerprint':fingerprint,'error':error})
    elif action in ('managed_read','managed_save'):
        root=safe_path(pathlib.Path(req['root']));path=safe_path(pathlib.Path(req['path']))
        if path!=root and root not in path.parents: raise ValueError('File outside inventory root')
        if not path.is_file() or path.stat().st_size>4*1024*1024: raise ValueError('Select an existing text file below 4 MiB')
        data=path.read_bytes()
        if b'\x00' in data: raise ValueError('Binary file')
        content=data.decode('utf-8');revision=hashlib.sha256(data).hexdigest()
        if action=='managed_save':
            if revision!=req['expected']: raise ValueError('File changed since last scan')
            replacement=req['content'].encode('utf-8')
            if len(replacement)>4*1024*1024 or b'\x00' in replacement: raise ValueError('Invalid text')
            fd,staged=tempfile.mkstemp(prefix='.hk-text-',dir=path.parent)
            try:
                with os.fdopen(fd,'wb') as f:
                    f.write(replacement);f.flush();os.fsync(f.fileno())
                os.chmod(staged,path.stat().st_mode&0o777)
                if safe_path(path)!=path or hashlib.sha256(path.read_bytes()).hexdigest()!=revision: raise ValueError('File changed while saving')
                os.replace(staged,path)
            finally:
                if os.path.exists(staged): os.unlink(staged)
            content=req['content'];revision=hashlib.sha256(replacement).hexdigest()
        result={'path':str(path),'content':content,'revision':revision,'captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    elif action=='expand_path':
        result=str(pathlib.Path(req['path']).expanduser())
    elif action=='browse_paths':
        path=pathlib.Path(req['path']).expanduser()
        while not path.is_dir():
            if path.is_symlink(): raise ValueError('Symlink paths are not traversed')
            if path.parent==path: raise ValueError('No accessible parent directory')
            path=path.parent
        path=safe_path(path)
        entries,truncated=metadata_entries(path)
        result={'path':str(path),'parent':str(path.parent) if path.parent!=path else None,'truncated':truncated,'entries':[{'name':e.name,'path':e.path,'is_directory':e.is_dir(follow_symlinks=False)} for e in entries if e.is_dir(follow_symlinks=False) or (e.is_file(follow_symlinks=False) and pathlib.Path(e.name).suffix in ('.md','.mdc','.toml','.json','.jsonc','.yaml','.yml','.txt','.sh','.js','.ts'))]}
    elif action=='discover_projects':
        root=safe_path(pathlib.Path(req['root']))
        if not root.is_dir(): raise ValueError('Select a directory')
        skipped={'.git','.hg','.svn','node_modules','target','dist','build','vendor','.venv','venv','__pycache__','.next','.cache'}
        result=[];pending=[(root,0)];visited=0
        while pending and visited<10000:
            path,depth=pending.pop();visited+=1
            try: entries,_=metadata_entries(path)
            except (OSError,ValueError): continue
            for spec in req['specifications']:
                components=[];seen=set()
                for component in spec['components']:
                    for index,p in enumerate(component_paths(path,component['relative'])):
                        if index>=1000: break
                        try: safe_path(p)
                        except (OSError,ValueError): continue
                        if component['is_directory'] and not p.is_dir() or not component['is_directory'] and not p.is_file(): continue
                        if component['kind']=='skills' and not contains_skill_files(p): continue
                        relative=p.relative_to(path).as_posix()
                        if relative not in seen: components.append(dict(component,relative=relative));seen.add(relative)
                if components: result.append({'root':str(path),'agent':spec['agent'],'components':components})
            if depth<12:
                pending.extend((pathlib.Path(e.path),depth+1) for e in reversed(entries) if e.is_dir(follow_symlinks=False) and e.name not in skipped)
                pending=pending[:10000-visited]
    elif action=='path_access':
        result=[]
        for item in req['paths']:
            try:
                path=pathlib.Path(item['path']);guard(path)
                if not item.get('is_directory',True):
                    if not path.is_file():
                        path.stat()
                        raise ValueError('Expected a file')
                    result.append({'state':'available','count':None,'error':None});continue
                count=0
                with os.scandir(path) as entries:
                    for index,entry in enumerate(entries):
                        if index>=10001: break
                        if not item.get('count') or (entry.is_dir(follow_symlinks=False) and ((pathlib.Path(entry.path)/'SKILL.md').is_file() or (pathlib.Path(entry.path)/'SKILL.md.disabled').is_file())): count+=1
                result.append({'state':'empty' if item.get('count') and count==0 else 'available','count':count if item.get('count') else None,'error':None})
            except Exception as exc:
                state='missing' if isinstance(exc,FileNotFoundError) else 'denied' if isinstance(exc,PermissionError) else 'error'
                result.append({'state':state,'count':None,'error':str(exc)})
    elif action=='probe':
        home=pathlib.Path.home()
        settings={}
        db=home/'.harnesskit/metadata.db'
        if db.is_file():
            import sqlite3
            with sqlite3.connect(db.as_uri()+'?mode=ro',uri=True) as connection:
                settings={name:(custom,bool(enabled)) for name,custom,enabled in connection.execute('SELECT name,custom_path,enabled FROM agent_settings')}
        roots=[];disabled=[];catalog=[]
        specifications=[('claude','.claude',['skills']),('codex','.codex',['skills']),('gemini','.gemini',['skills']),('cursor','.cursor',['skills']),('antigravity','.gemini/config',['skills']),('copilot','.copilot',['skills']),('windsurf','.codeium/windsurf',['skills']),('opencode','.config/opencode',['skills']),('hermes','.hermes',['skills/local','skills']),('kiro','.kiro',['skills']),('omp','.omp',['agent/skills','agent/managed-skills']),('dsh','.dsh',['skills']),('grok','.grok',['skills']),('qoder-cn','.qoder-cn',['skills']),('openclaw','.openclaw',['skills','workspace/skills','workspace/.agents/skills']),('penguin','.penguin/data',[]),('pi','.pi',['agent/skills'])]
        shared={'codex','gemini','cursor','copilot','windsurf','opencode','omp','dsh','qoder-cn','openclaw','pi'}
        for provider,directory,relative_roots in specifications:
            custom,enabled=settings.get(provider,(None,True))
            base=pathlib.Path(custom) if custom else home/directory
            paths=[base/relative for relative in relative_roots]
            if provider=='penguin': paths=sorted(base.glob('*/agents/*/agent_state/skills')) or [base/'skills']
            if provider=='antigravity':
                paths.append(home/'.gemini/antigravity/skills')
                if (home/'.gemini/antigravity-cli').is_dir(): paths.extend([home/'.gemini/antigravity-cli/skills',home/'.gemini/skills'])
            if provider in shared: paths.append(home/'.agents/skills')
            for path in paths:
                error=None
                try:
                    if not base.is_absolute(): raise ValueError('Provider custom path must be absolute')
                    guard(path)
                    present=path.is_dir()
                    detected=base.is_dir()
                except (ValueError,OSError) as exc: error=str(exc);present=False;detected=False
                catalog.append({'name':provider,'home':str(base),'root':str(path),'detected':detected,'enabled':enabled,'error':error})
                if present: roots.append((provider,str(path)))
            if not enabled: disabled.append(provider)
        result={'home':str(home),'os':platform.system().lower(),'providers':roots,'disabled_providers':disabled,'catalog':catalog}
    else:
        guard(pathlib.Path(req['root']))
        root=pathlib.Path(req['root']).resolve(strict=True)
        if not root.is_dir(): raise ValueError('Provider root is not a directory')
        if action=='origins':
            result={}
            lock=root.parent/'.skill-lock.json'
            if lock.is_file() and lock.stat().st_size<4*1024*1024:
                try:
                    for name,entry in json.loads(lock.read_text()).get('skills',{}).items():
                        url=entry.get('sourceUrl') or entry.get('source')
                        if isinstance(url,str): result[name]=url
                except (ValueError,OSError): pass
            for child in root.iterdir():
                if child.name in result or child.name.startswith('.hk-'): continue
                if not ((child/'SKILL.md').is_file() or (child/'SKILL.md.disabled').is_file()): continue
                for parent in [child,*child.parents]:
                    config=parent/'.git/config'
                    if config.is_file() and config.stat().st_size<1024*1024:
                        try:
                            parser=configparser.RawConfigParser(strict=False);parser.read(config)
                            url=parser.get('remote "origin"','url',fallback=None)
                            if url: result[child.name]=url
                        except (ValueError,configparser.Error): pass
                        break
        elif action in ('text_read','text_save'):

            name=req['file']
            if name not in ('config.toml','AGENTS.md','CLAUDE.md','GEMINI.md','settings.json','config.json','mcp.json'):
                raise ValueError('Unsupported provider configuration file')
            parent=root.parent
            path=parent/name
            if path.is_symlink() or not path.is_file() or path.stat().st_size>4*1024*1024: raise ValueError('Select an existing text file below 4 MiB')
            data=path.read_bytes()
            if b'\x00' in data: raise ValueError('Binary file')
            content=data.decode('utf-8');revision=hashlib.sha256(data).hexdigest()
            if action=='text_save':
                if revision!=req['expected']: raise ValueError('File changed since comparison opened')
                replacement=req['content'].encode('utf-8')
                if len(replacement)>4*1024*1024 or b'\x00' in replacement: raise ValueError('Invalid text')
                history=pathlib.Path.home()/'.harnesskit/comparison-history'
                guard(history)
                history.mkdir(parents=True,exist_ok=True);history.chmod(0o700)
                backup=history/revision
                try:
                    with backup.open('xb') as f:
                        os.chmod(backup,0o600);f.write(data);f.flush();os.fsync(f.fileno())
                except FileExistsError: pass
                fd,staged=tempfile.mkstemp(prefix='.hk-text-',dir=parent)
                try:
                    with os.fdopen(fd,'wb') as f:
                        f.write(replacement);f.flush();os.fsync(f.fileno())
                    os.chmod(staged,path.stat().st_mode&0o777)
                    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=revision: raise ValueError('File changed while saving')
                    os.replace(staged,path)
                finally:
                    if os.path.exists(staged): os.unlink(staged)
                content=req['content'];revision=hashlib.sha256(replacement).hexdigest()
            result={'path':str(path),'content':content,'revision':revision,'captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
        elif action=='list':

            for j in root.glob('.hk-transaction-*.json'):
                info=json.loads(j.read_text());name=info.get('name')
                if name and '/' not in name and '\\' not in name: recover(root/name)
            result=sorted(p.name for p in root.iterdir() if not p.name.startswith('.hk-') and ((p/'SKILL.md').is_file() or (p/'SKILL.md.disabled').is_file()))
        else:
            name=req['name']
            if not name or '/' in name or '\\' in name or name in ('.','..'): raise ValueError('Invalid skill name')
            path=root/name
            if action=='read': result=folder(path)
            elif action=='write':
                if signature(folder(path))!=req.get('expected'): raise ValueError('Target changed; rescan required')
                snapshot=req.get('folder');stage=pathlib.Path(tempfile.mkdtemp(prefix='.hk-stage-',dir=root));old=root/('.hk-old-'+stage.name)
                try:
                    if snapshot:
                        for relative in snapshot.get('directories',[]):
                            rel=pathlib.PurePosixPath(relative)
                            if rel.is_absolute() or '..' in rel.parts or '\\' in relative or ':' in relative: raise ValueError('Unsafe directory path')
                            (stage/rel).mkdir(parents=True,exist_ok=True)
                        for relative,data in snapshot['files'].items():
                            rel=pathlib.PurePosixPath(relative)
                            if rel.is_absolute() or '..' in rel.parts or '\\' in relative or ':' in relative: raise ValueError('Unsafe snapshot path')
                            item=stage/rel;item.parent.mkdir(parents=True,exist_ok=True);item.write_bytes(bytes(data))
                            if os.name!='nt': item.chmod(snapshot.get('modes',{}).get(relative,0o644)&0o777)
                        if signature(folder(stage))!=snapshot['fingerprint']: raise ValueError('Snapshot checksum mismatch')
                    if signature(folder(path))!=req.get('expected'): raise ValueError('Target changed while staging')
                    journal=root/('.hk-transaction-'+hashlib.sha256(name.encode()).hexdigest()+'.json')
                    journal.write_text(json.dumps({'old':old.name,'name':name}))
                    if path.exists(): path.rename(old)
                    try:
                        if snapshot: stage.rename(path)
                    except Exception:
                        if old.exists(): old.rename(path)
                        raise
                    journal.unlink()
                    if old.exists(): shutil.rmtree(old)
                    result=True
                finally:
                    if stage.exists(): shutil.rmtree(stage)
            else: raise ValueError('Unknown operation')
    json.dump({'result':result},sys.stdout,ensure_ascii=False,separators=(',',':'))
except Exception as e:
    json.dump({'error':str(e)},sys.stdout)
    sys.exit(1)
