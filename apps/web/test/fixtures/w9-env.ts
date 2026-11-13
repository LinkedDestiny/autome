import captured from './w9-env-replies.json' with { type: 'json' };
import type { EnvData, EnvSnapshot, InstallRecipe } from '../../app/components/env-model.ts';
export const recipe: InstallRecipe = { ...captured.recipes.claude, prerequisite_ok: true };
export const snapshot: EnvSnapshot = {probed:true,probing:false,generation:1,environment:{components:[
  {component:'git',present:true,version:'git version 1',path:'/bin/git',login:{login:'not_applicable'}},
  {component:'claude',present:true,version:'2',path:'/bin/claude',login:{login:'ok',account_hint:'user@example.com'}},
  {component:'codex',present:true,version:'3',path:'/bin/codex',login:{login:'expired'}},
  {component:'iterm2',present:false,version:null,path:null,login:{login:'unknown',detail:'检测原文'}},
]},problems:[{component:'codex',present:true,login:{login:'expired'}},{component:'iterm2',present:false,login:{login:'unknown'}}]};
export const envData: EnvData = {snapshot,recipes:{claude:recipe},errors:{}};
