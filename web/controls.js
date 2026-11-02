'use strict';
// Form-associated controls: no native select, checkbox or radio widgets.
(()=>{
 const t=value=>window.uiText?window.uiText(value):value;
 const css=`
 [hidden]{display:none!important}:host{display:block;min-width:0;color:var(--text);font:inherit}*{box-sizing:border-box}
 button,input{font:inherit;color:inherit}button{cursor:pointer}button:disabled{cursor:not-allowed;opacity:.45}
 button:focus-visible,input:focus-visible{outline:3px solid var(--accent);outline-offset:2px}
 .trigger{display:flex;align-items:center;justify-content:space-between;gap:12px;width:100%;min-height:40px;border:1px solid var(--line);border-radius:9px;padding:9px 12px;background:var(--surface);text-align:left;font-size:14px;transition:border-color .15s,background .15s}
 .trigger:hover:not(:disabled),.trigger[aria-expanded=true]{border-color:var(--accent);background:var(--accent-fill)}
 .caption{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.chevron{width:14px;height:14px;flex:none;transition:transform .15s}.trigger[aria-expanded=true] .chevron{transform:rotate(180deg)}
 .menu{position:fixed;inset:auto;margin:0;padding:6px;border:1px solid var(--line);border-radius:12px;color:var(--text);background:var(--surface);box-shadow:0 12px 40px #0003;overflow:hidden;font:14px/1.5 Inter,ui-sans-serif,system-ui,sans-serif;max-width:calc(100vw - 16px)}
 .search{display:block;width:100%;padding:10px 11px;margin-bottom:6px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);font-size:13px}
 .rows{max-height:260px;overflow:auto;overscroll-behavior:contain}.option{display:flex;align-items:center;justify-content:space-between;gap:12px;width:100%;padding:10px 11px;border:0;border-radius:7px;background:transparent;text-align:left;font-size:13px;line-height:1.45;overflow-wrap:anywhere}
 .option:hover,.option.active{background:var(--accent-fill)}.option[aria-selected=true]{color:var(--accent);font-weight:600}.check{width:16px;flex:none}.empty{padding:12px;color:var(--muted);font-size:13px}
 :host(.language-select){width:76px;padding:0}:host(.language-select) .trigger{min-height:36px;padding:7px 10px;gap:8px;font-size:12px;font-weight:600}:host(.language-select) .caption{flex:1;overflow:visible;text-overflow:clip;text-align:center}:host(.language-select) .chevron{width:12px;height:12px}
 :host(#workflow-select) .trigger{font-size:17px;font-weight:500;background:transparent;min-height:38px;border:0;border-bottom:1px solid var(--line);border-radius:0;padding:4px 8px 4px 0}
 :host(#workflow-select) .trigger:hover:not(:disabled),:host(#workflow-select) .trigger[aria-expanded=true]{border-bottom-color:var(--accent)}
 :host(#workflow-select){width:auto;max-width:100%}
 .workflow-sizer{display:grid;height:0;overflow:hidden;visibility:hidden;font-size:17px;font-weight:500}
 .workflow-sizer span{grid-area:1/1;white-space:pre;padding-right:36px}
 :host([disabled]){opacity:.6}
 @media(max-width:550px){:host(#workflow-select) .trigger,.workflow-sizer{font-size:14px}}
 @media(prefers-reduced-motion:reduce){*{transition:none!important}}
 `;
 let current=null,sequence=0;
 class FormControl extends HTMLElement{
  static formAssociated=true;
  constructor(){super();this.internals=this.attachInternals();this.root=this.attachShadow({mode:'open',delegatesFocus:true});this._fieldsetDisabled=false;this.addEventListener('click',e=>{if(e.composedPath()[0]===this&&!this.disabled)this.button?.click()});}
  get form(){return this.internals.form}get name(){return this.getAttribute('name')||''}
  get disabled(){return this.hasAttribute('disabled')||this._fieldsetDisabled}set disabled(value){this.toggleAttribute('disabled',!!value)}
  get required(){return this.hasAttribute('required')}set required(value){this.toggleAttribute('required',!!value)}
  get validity(){return this.internals.validity}get validationMessage(){return this.internals.validationMessage}get willValidate(){return this.internals.willValidate}
  checkValidity(){return this.internals.checkValidity()}reportValidity(){return this.internals.reportValidity()}
  focus(options){this.button?.focus(options)}
  formDisabledCallback(value){this._fieldsetDisabled=value;this.sync()}
  emit(){this.dispatchEvent(new Event('input',{bubbles:true,composed:true}));this.dispatchEvent(new Event('change',{bubbles:true,composed:true}))}
  label(){return this.getAttribute('aria-label')||Array.from(this.internals.labels||[]).map(label=>Array.from(label.childNodes).filter(n=>n!==this).map(n=>n.textContent).join(' ').trim()).join(' ')||this.name}
 }
 class Picker extends FormControl{
  static observedAttributes=['disabled','required','aria-label'];
  constructor(){super();this._value=undefined;this.active=-1;this.uid='picker-'+(++sequence);
   this.root.innerHTML=`<style>${css}</style><button type="button" class="trigger" role="combobox" aria-haspopup="listbox" aria-expanded="false" aria-controls="${this.uid}"><span class="caption"></span><svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><path d="m6 9 6 6 6-6"/></svg></button><div class="menu" popover="manual"><input class="search" type="search" autocomplete="off"><div class="rows" role="listbox" id="${this.uid}"></div></div>`;
   this.button=this.root.querySelector('button');this.menu=this.root.querySelector('.menu');this.search=this.root.querySelector('input');this.rows=this.root.querySelector('.rows');
   this.button.onclick=()=>this.opened?this.close():this.open();this.button.onkeydown=e=>this.key(e);
   this.search.oninput=e=>{e.stopPropagation();this.active=0;this.draw()};this.search.onchange=e=>e.stopPropagation();this.search.onkeydown=e=>this.key(e);
   this.rows.onclick=e=>{const row=e.target.closest('[data-index]');if(row)this.choose(Number(row.dataset.index))};
   this.rows.onkeydown=e=>this.key(e);
   this.observer=new MutationObserver(()=>this.sync());
   this.outside=e=>{if(this.opened&&!e.composedPath().includes(this))this.close()};
   this.reposition=()=>{if(this.opened)this.position()};this.localize=()=>{this.sync();if(this.opened)this.draw()};
  }
  connectedCallback(){this.observer.observe(this,{subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:['selected','disabled','hidden','value']});document.addEventListener('pointerdown',this.outside,true);window.addEventListener('resize',this.reposition);document.addEventListener('scroll',this.reposition,true);window.addEventListener('nodo-language-changed',this.localize);this.sync()}
  disconnectedCallback(){clearTimeout(this.typeTimer);this.close(false);this.observer.disconnect();document.removeEventListener('pointerdown',this.outside,true);window.removeEventListener('resize',this.reposition);document.removeEventListener('scroll',this.reposition,true);window.removeEventListener('nodo-language-changed',this.localize)}
  attributeChangedCallback(){if(this.button)this.sync()}
  get options(){return Array.from(this.querySelectorAll('jui-option'))}
  get value(){this.resolve();return this._value||''}set value(value){this._value=String(value??'');this.sync()}
  resolve(){const options=this.options;if(!options.some(o=>o.getAttribute('value')===this._value))this._value=(options.find(o=>o.hasAttribute('selected'))||options[0])?.getAttribute('value')||''}
  labelFor(option){return option?(option.hasAttribute('data-no-i18n')?option.textContent:t(option.textContent)).trim():''}
  sync(){this.resolve();const option=this.options.find(o=>o.getAttribute('value')===this._value);this.internals.setFormValue(this._value);this.button.disabled=this.disabled;this.button.setAttribute('aria-label',t(this.label()));this.root.querySelector('.caption').textContent=this.labelFor(option)||'—';this.internals.setValidity(this.required&&!this._value?{valueMissing:true}:{},this.required&&!this._value?t('Seleziona un’opzione'):undefined,this.button);if(this.id==='workflow-select')this.sizeWorkflow();if(this.disabled)this.close(false);if(this.opened)this.draw()}
  sizeWorkflow(){
   // Intrinsic text widths also react to font loading and viewport changes.
   let sizer=this.root.querySelector('.workflow-sizer');
   if(!sizer){sizer=document.createElement('div');sizer.className='workflow-sizer';sizer.setAttribute('aria-hidden','true');this.root.append(sizer)}
   sizer.replaceChildren(...this.options.map(option=>{const span=document.createElement('span');span.textContent=this.labelFor(option);return span}));
  }
  get opened(){return this.menu.matches(':popover-open')}
  visible(){const term=this.search.value.toLocaleLowerCase();return this.options.map((option,index)=>({option,index})).filter(({option})=>!option.hidden&&(!term||this.labelFor(option).toLocaleLowerCase().includes(term)))}
  draw(){const items=this.visible();if(this.active>=items.length)this.active=items.length-1;this.rows.replaceChildren();for(const [i,{option,index}] of items.entries()){const row=document.createElement('button');row.type='button';row.className='option'+(i===this.active?' active':'');row.dataset.index=index;row.id=this.uid+'-'+index;row.setAttribute('role','option');row.setAttribute('aria-selected',String(option.getAttribute('value')===this._value));row.tabIndex=-1;row.disabled=option.hasAttribute('disabled');const text=document.createElement('span');text.textContent=this.labelFor(option);const check=document.createElement('span');check.className='check';check.textContent=option.getAttribute('value')===this._value?'✓':'';row.append(text,check);this.rows.append(row)}if(!items.length){const empty=document.createElement('div');empty.className='empty';empty.textContent=t('Nessun risultato');this.rows.append(empty)}const row=this.rows.children[this.active];for(const el of [this.search,this.button]){if(row?.id)el.setAttribute('aria-activedescendant',row.id);else el.removeAttribute('aria-activedescendant')}this.search.placeholder=t('Cerca…');this.search.setAttribute('aria-label',t('Cerca…'));this.rows.setAttribute('aria-label',t(this.label()));}
  open(){if(this.disabled||!this.options.length)return;if(current&&current!==this)current.close(false);current=this;this.search.value='';this.search.hidden=this.options.filter(o=>!o.hidden).length<7;this.active=this.visible().findIndex(({option})=>option.getAttribute('value')===this._value);this.draw();this.menu.showPopover();this.button.setAttribute('aria-expanded','true');this.position();if(!this.search.hidden)this.search.focus();else this.button.focus()}
  position(){const rect=this.button.getBoundingClientRect(),margin=8;this.menu.style.width=Math.min(Math.max(rect.width,230),innerWidth-16)+'px';const availableBelow=innerHeight-rect.bottom-margin,availableAbove=rect.top-margin;const above=availableBelow<220&&availableAbove>availableBelow;const maxHeight=Math.max(80,(above?availableAbove:availableBelow)-12);this.rows.style.maxHeight=Math.min(260,maxHeight-(this.search.hidden?0:48))+'px';const height=this.menu.getBoundingClientRect().height;this.menu.style.left=Math.max(margin,Math.min(rect.left,innerWidth-this.menu.offsetWidth-margin))+'px';this.menu.style.top=(above?Math.max(margin,rect.top-height-6):rect.bottom+6)+'px'}
  close(focus=true){if(this.opened)this.menu.hidePopover();this.button.setAttribute('aria-expanded','false');if(current===this)current=null;if(focus&&this.isConnected)this.button.focus()}
  choose(index){const option=this.options[index];if(!option||option.hasAttribute('disabled'))return;const changed=this.value!==option.getAttribute('value');this.value=option.getAttribute('value');this.close();if(changed)this.emit()}
  key(e){if(['ArrowDown','ArrowUp','Home','End','Enter',' ','Escape','Tab'].includes(e.key)){if(e.key==='Tab'){this.close(false);return}if(e.key==='Escape'&&this.opened){e.preventDefault();e.stopPropagation();this.close();return}if(e.key==='Escape')return;if(e.key===' '&&e.target===this.search)return;e.preventDefault();e.stopPropagation();if(!this.opened){this.open();return}const items=this.visible();if(e.key==='Enter'||e.key===' '){if(items[this.active])this.choose(items[this.active].index);return}if(e.key==='Home')this.active=0;else if(e.key==='End')this.active=items.length-1;else this.active=(this.active+(e.key==='ArrowDown'?1:-1)+items.length)%items.length;this.draw();this.rows.children[this.active]?.scrollIntoView({block:'nearest'})}else if(e.key.length===1&&!e.ctrlKey&&!e.metaKey){if(!this.opened)this.open();if(!this.search.hidden){if(e.target!==this.search){this.search.value=e.key;this.active=0;this.draw();e.preventDefault()}}else{clearTimeout(this.typeTimer);this.typeBuffer=(this.typeBuffer||'')+e.key.toLocaleLowerCase();this.active=this.visible().findIndex(({option})=>this.labelFor(option).toLocaleLowerCase().startsWith(this.typeBuffer));this.typeTimer=setTimeout(()=>{this.typeBuffer=''},700);this.draw();e.preventDefault()}}}
  formResetCallback(){this._value=undefined;this.sync()}formStateRestoreCallback(value){this.value=value}
 }
 class Switch extends FormControl{
  static observedAttributes=['checked','disabled','value','required','aria-label'];
  constructor(){super();this.root.innerHTML=`<style>${css}:host{display:inline-flex;flex:none}.toggle{border:0;border-radius:20px;width:30px;height:18px;min-height:0;padding:2px;background:var(--muted);display:inline-flex;align-items:center;transition:background .15s}.toggle::after{content:'';width:14px;height:14px;border-radius:50%;background:var(--surface);box-shadow:0 1px 3px #0002;transition:transform .15s}.toggle[aria-checked=true]{background:var(--accent)}.toggle[aria-checked=true]::after{transform:translateX(12px)}</style><button type="button" class="toggle" role="switch" aria-checked="false"></button>`;this.button=this.root.querySelector('button');this.button.onclick=()=>{if(this.disabled)return;if(this.hasAttribute('exclusive')){if(this.checked)return;for(const other of this.form?.querySelectorAll('jui-switch[exclusive]')||[])if(other!==this&&other.name===this.name)other.checked=false;this.checked=true}else this.checked=!this.checked;this.emit()};this.localize=()=>this.sync()}
  connectedCallback(){if(this._initialChecked===undefined)this._initialChecked=this.checked;window.addEventListener('nodo-language-changed',this.localize);this.sync()}disconnectedCallback(){window.removeEventListener('nodo-language-changed',this.localize)}
  attributeChangedCallback(){if(this.button)this.sync()}
  get checked(){return this.hasAttribute('checked')}set checked(value){this.toggleAttribute('checked',!!value)}
  get value(){return this.getAttribute('value')||'on'}set value(value){this.setAttribute('value',value)}
  sync(){this.internals.setFormValue(this.checked?this.value:null);this.button.disabled=this.disabled;this.button.setAttribute('aria-checked',String(this.checked));this.button.setAttribute('aria-label',t(this.label()));this.internals.setValidity({})}
  formResetCallback(){this.checked=this._initialChecked}formStateRestoreCallback(value){this.checked=value!==null}
 }
 window.bindModelSuggestions=(field,catalog,models)=>{
  const menu=document.createElement('div');menu.className='model-suggestions';menu.setAttribute('popover','manual');menu.setAttribute('role','listbox');menu.id='model-suggestions-'+(++sequence);catalog.append(menu);
  field.setAttribute('role','combobox');field.setAttribute('aria-autocomplete','list');field.setAttribute('aria-controls',menu.id);field.setAttribute('aria-expanded','false');
  let rows=[],active=-1;
  const close=()=>{if(menu.matches(':popover-open'))menu.hidePopover();field.setAttribute('aria-expanded','false');field.removeAttribute('aria-activedescendant');active=-1};
  const position=()=>{const rect=field.getBoundingClientRect();menu.style.width=Math.min(Math.max(rect.width,250),innerWidth-16)+'px';menu.style.left=Math.max(8,Math.min(rect.left,innerWidth-menu.offsetWidth-8))+'px';const below=innerHeight-rect.bottom;menu.style.maxHeight=Math.min(280,Math.max(below,rect.top)-20)+'px';menu.style.top=(below>200?rect.bottom+6:Math.max(8,rect.top-menu.offsetHeight-6))+'px'};
  const choose=model=>{field.value=model.id;close();field.dispatchEvent(new Event('input',{bubbles:true}));field.dispatchEvent(new Event('change',{bubbles:true}));field.focus()};
  const draw=()=>{const term=field.value.toLocaleLowerCase();rows=models.filter(m=>(m.id+' '+m.name).toLocaleLowerCase().includes(term)).slice(0,80);menu.replaceChildren();rows.forEach((model,i)=>{const button=document.createElement('button');button.type='button';button.id=menu.id+'-'+i;button.setAttribute('role','option');button.setAttribute('aria-selected',String(i===active));button.setAttribute('data-no-i18n','');button.textContent=model.name+' · '+model.id;button.onpointerdown=e=>e.preventDefault();button.onclick=()=>choose(model);menu.append(button)});if(!rows.length){close();return}menu.showPopover();field.setAttribute('aria-expanded','true');position()};
  const key=e=>{if(e.key==='Escape'&&menu.matches(':popover-open')){e.preventDefault();e.stopPropagation();close()}else if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();e.stopPropagation();if(!menu.matches(':popover-open'))draw();if(!rows.length)return;active=(active+(e.key==='ArrowDown'?1:-1)+rows.length)%rows.length;Array.from(menu.children).forEach((row,i)=>row.setAttribute('aria-selected',String(i===active)));field.setAttribute('aria-activedescendant',menu.children[active].id);menu.children[active].scrollIntoView({block:'nearest'})}else if(e.key==='Enter'&&active>=0&&menu.matches(':popover-open')){e.preventDefault();e.stopPropagation();choose(rows[active])}else if(e.key==='Tab')close()};
  const outside=e=>{if(e.target!==field&&!menu.contains(e.target))close()};
  field.addEventListener('input',draw);field.addEventListener('focus',draw);field.addEventListener('blur',close);field.addEventListener('keydown',key);document.addEventListener('pointerdown',outside,true);
  let disposed=false;
  const lifecycle=new MutationObserver(()=>{if(!field.isConnected)dispose()});
  const reposition=()=>{if(menu.matches(':popover-open'))position()};
  const dispose=()=>{if(disposed)return;disposed=true;close();lifecycle.disconnect();field.removeEventListener('input',draw);field.removeEventListener('focus',draw);field.removeEventListener('blur',close);field.removeEventListener('keydown',key);document.removeEventListener('pointerdown',outside,true);document.removeEventListener('scroll',reposition,true);window.removeEventListener('resize',reposition);for(const attr of ['role','aria-autocomplete','aria-controls','aria-expanded','aria-activedescendant'])field.removeAttribute(attr)};
  lifecycle.observe(document.body,{childList:true,subtree:true});document.addEventListener('scroll',reposition,true);window.addEventListener('resize',reposition);
  return dispose;
 };
 customElements.define('jui-select',Picker);customElements.define('jui-switch',Switch);
})();
