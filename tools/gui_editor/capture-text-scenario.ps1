# Read-only independent CURRENT_CITY_NAME selection inputs, using verified island
# lookup and settlement-string contracts in findings/Ghidra. No cached GUI text read.
param([string]$Name='text-scenario')
$ErrorActionPreference='Stop'
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Use a simple reference name'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$game=Get-Process -Name Addon -ErrorAction Stop | Where-Object {$_.Path -eq (Join-Path $repo 'Anno 1404 Gold Edition/Addon.exe')} | Select-Object -First 1
if(-not $game){throw 'Repo-local game not visible; use elevated read-only inspection'}
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoTextScenario {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr p,IntPtr address,byte[] bytes,int count,out IntPtr got);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr p);
 public class Scenario {public int IslandId,Player,UnderCursorIsland,MirrorPlayer;public uint GameEngine,Island,Settlement;public string Name;public List<string> Names;public int[] PrimaryScores,SecondaryCounts;}
 static byte[] B(IntPtr p,uint a,int n){if(n<0||n>512)throw new Exception("Read bound");var b=new byte[n];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,n,out got)||got.ToInt64()!=n)throw new Exception("Read failed at "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(B(p,a,4),0);}
 static string NameAt(IntPtr p,uint settlement){
  uint end=U(p,settlement+0x20),storage=U(p,settlement+0x24),begin=storage==settlement+0x20?settlement:U(p,settlement);
  if(end<begin||end-begin>512||(end-begin)%2!=0)throw new Exception("Settlement string bound");
  var bytes=B(p,begin,(int)(end-begin));
  if(U(p,settlement+0x20)!=end||U(p,settlement+0x24)!=storage||(storage!=settlement+0x20&&U(p,settlement)!=begin))throw new Exception("Settlement string changed");
  return new UnicodeEncoding(false,false,true).GetString(bytes);
 }
 static int CountEntries(IntPtr p,uint island,int player){
  int result=0;
  for(int group=0;group<4;group++)for(int tier=0;tier<6;tier++){
   uint header=island+0x10+(uint)player*12+(uint)tier*0x168+(uint)group*0x870;
   var bytes=B(p,header,8);uint begin=BitConverter.ToUInt32(bytes,0),end=BitConverter.ToUInt32(bytes,4);
   if(end<begin||(end-begin)%4!=0||end-begin>4000000)throw new Exception("Vector count bound");
   result=checked(result+(int)((end-begin)/4));
  }
  return result;
 }
 public static Scenario Read(int pid){
  var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("Read-only OpenProcess failed");
  try{
   var s=new Scenario();s.GameEngine=U(p,0x012D92E8);if(s.GameEngine==0)throw new Exception("Game engine missing");
   s.IslandId=(int)U(p,s.GameEngine+0xe8);s.Player=(int)U(p,s.GameEngine+0xec);s.UnderCursorIsland=(int)U(p,0x012D54F8);s.MirrorPlayer=(int)U(p,0x012D54F4);
   if(s.IslandId<=0)throw new Exception("No last-nonzero island scenario");
   uint gop=U(p,s.GameEngine+0x90),header=gop+0x7f5c,count=U(p,header+0x10),node=U(p,header+4);
   if(count==0||count>=4096)throw new Exception("Island tree count bound");
   var seen=new HashSet<uint>();
   while(node!=0&&node!=header){
    if(!seen.Add(node)||seen.Count>64)throw new Exception("Island lookup bound");
    int key=(int)U(p,node+0x10);if(key==s.IslandId){s.Island=U(p,node+0x14);break;}node=U(p,node+(s.IslandId<key?8u:12u));
   }
   if(s.Island==0)throw new Exception("Island not found");
   s.Names=new List<string>();s.PrimaryScores=new int[8];s.SecondaryCounts=new int[30];
   for(int player=0;player<30;player++){
    s.Names.Add(NameAt(p,s.Island+0x39b8+(uint)player*0x30));s.SecondaryCounts[player]=CountEntries(p,s.Island,player);
    if(player<8)s.PrimaryScores[player]=(int)U(p,s.Island+0x2cc0+(uint)player*4);
   }
   s.Settlement=s.Player>=0&&s.Player<30?s.Island+0x39b8+(uint)s.Player*0x30:0;s.Name=s.Settlement==0?"":s.Names[s.Player];
   if(U(p,0x012D92E8)!=s.GameEngine||(int)U(p,s.GameEngine+0xe8)!=s.IslandId||(int)U(p,s.GameEngine+0xec)!=s.Player||U(p,s.GameEngine+0x90)!=gop)throw new Exception("Scenario changed during capture");
   return s;
  }finally{CloseHandle(p);}
 }
}
'@
$scenario=[AnnoTextScenario]::Read($game.Id)
$record=@{schema=3;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();source=$scenario;scope='Independent CURRENT_CITY_NAME inputs: game-engine last-nonzero island+E8, local player+EC, GOP+90,30 settlement names,8 primary scores and30 secondary vector-entry counts. Cursor/mirror globals and local Name are diagnostic only. Selection occurs in the standalone Rust resolver with original localizationGUID140038 fallback. Non-atomic snapshot; no cached GUI text, game calls or writes.'}
$destination=Join-Path $PSScriptRoot '.tmp/references'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
[IO.File]::WriteAllText((Join-Path $destination "$Name-scenario.json"),($record|ConvertTo-Json -Depth 6))
$record|ConvertTo-Json -Depth 6
