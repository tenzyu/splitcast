# SplitCast アーキテクチャ実装・検証 · 2026-11-20

設計 `f089da89995da9d8ce2ba515a01eb814a68c721d` を実装した。B は
ペアの取得世代と解放確認を変更し、C/D は制作側の監視を Studio コマンドから独立させた。
E の同期判定が不合格だったため、制作マイクは専用 PulseAudio を維持した。
[設計](architecture-convergence-20261120.md) · [E の測定と再現](unit-e-obs-pcm-20261120.md)

## A · 基準と再現

`git rev-parse HEAD HEAD^` は設計 `f089da89995da9d8ce2ba515a01eb814a68c721d` と親
`ca213f8cc9b5009b603528042fea0a6eb2b271b0` を返した。
`git diff --name-status HEAD^ HEAD` は `A docs/evidence/architecture-convergence-20261120.md` の1行。
A の試験追加 `31901d10e9f13c16c9bdcb147dfd74dfcf28f014`、tree
`361287958cecaa0fea678a98b9b58905fffdc1a0`、snapshot
`24ce699087268745a3bc998f6c0bfcd5fdb6fc0ea3dd3c2b9efdd3493a3cc72c`。
Rust 1.95.0 `59807616e1fa2540724bfbac14d7976d7e4a3860`、
Cargo 1.95.0 `f2d3ce0bd`、Nix 2.34.9、Bubblewrap 0.11.2、x86_64-linux。
Cargo、Studio、flake の lock blob は順に
`cc2f9a4f013ba244411710a6250989bc331ae470`、
`0395f6fef977d74774ddc73f8ffc22e63b5fbb04`、
`026daf957b056470b8a1c991bdca5dd41129c63a`。
隔離プレフライトで IPv4/IPv6 の loopback のみ、`/dev/snd`、`/dev/dri`、`/dev/video0` の不在を確認。

```bash
isolated=(bwrap --unshare-all --die-with-parent --bind / / --dev /dev --proc /proc
  --tmpfs /tmp --tmpfs /run/user --setenv TMPDIR /tmp
  --unsetenv DISPLAY --unsetenv WAYLAND_DISPLAY --unsetenv SSH_AUTH_SOCK)
nix develop .#media-test -c "${isolated[@]}" cargo test --locked --offline \
  -p splitcast-rs-core --test two_agents production_held_scene_edit -- --exact --nocapture
nix develop .#media-test -c "${isolated[@]}" cargo run -p splitcast-xtask -- verify
nix develop .#media-test -c "${isolated[@]}" cargo run -p splitcast-xtask -- verify-virtual
```

- 直接の保持試験は終了0。100 ms の編集は適用され、Running の出力 byte がその後2回増加した。
  3,500 ms の編集も適用され、直後に Failed と `incoming media or OBS output stopped progressing` を観測した。
  PID と受信・OBS ポートの解放も確認した。fixture `two-agents-2758-0` / `two-agents-2758-1`。
- `verify` 初回は終了1。xtask `1795154132237395758` の `tests` が終了101。
  held edit が OBS 制御期限超過、開始応答保持中の Stop が500 ms 超、return fixture が観測期限超過、
  2 Agent の Start が I/O 期限超過。続く no-default-features を含む残り9項目は成功。
  同時ビルドがあった。個々の期限超過の原因は未確定。
- `verify-virtual` は同じ A の SHA で終了0、xtask `1795154438958948089` の14項目成功。
  旧 Prepare の再取得、同じ条件での旧 Start 拒否、待ち行列の Stop フェンスを再確認した。
  ソケットの先頭4 byte を相手側で読み取る試験が、正の送信前後で異なる解放責務を検証した。
  private-audio、generated-media、native-return、local-transport も実行された。

ローカル convergence `architecture-handoff-a-baseline` の contract は
`212c2af3cf2e767c8e0fb08af36d8b49b6909821f2064dba654a5ccec6df9dbe`。
順に `attempt-000001`、`attempt-000002`、`attempt-000003`。
最初の未登録 contract は物理3軸の記述が欠けて終了2。
検証記録の再利用は収録されたソースと環境の一致に従う。

次に `isolated+=(--setenv RUST_TEST_THREADS 2)` を加えて、同じ A で
`verify` と `verify-virtual` を実行した。両方終了0で、それぞれ10項目、14項目が成功。
ソフトウェアの期限値は変更していない。
contract `architecture-handoff-a-controlled` の digest
`bf7746b16255105dfdfa23d1de7f5499ea4539979948b63ca99a8f4a9dce4db8`。
`attempt-000001` / xtask `1795154543019153259`、
`attempt-000002` / xtask `1795154768689842407`。

## B · ペアの取得と解放

| 変更 | SHA |
| --- | --- |
| B の実装 | `6e9e513abd6ea46b34ee61055b8f19f0d3847053` |
| B 単独で遅延 Prepare 試験を直した検証用コミット | `b7f3c8e75746a0972b5da0b00c34e747591c2278` |
| 統合ブランチの同じ試験修正 | `2a9b077aa41bf0bd93272b0feec866e7d1616b74` |

ペアの `transport.version` はメディア有無とも 3。認証済み `routes` が
利用可能な Idle 世代を返す。キャプチャ側の nonce と合成した所有者を、
制作側の処理開始時に Preparing へ一度だけ確定する。Stop と期限も同じ
ロックで検査する。キャプチャ側は部分的な正の送信から解放責務を持つ。
即時 200 は同じ制作プロセスと所有者、202 の追跡はさらに同じ
`stop_id` を要する。明示的に確認できない場合は責務を保持する。
[制御契約](../virtual-control-design.md#remote-completion-and-uncertainty)

上の A の隔離配列に `--setenv RUST_TEST_THREADS 2` を加え、
`nix develop .#media-test -c "${isolated[@]}" cargo run -p splitcast-xtask -- verify`
と同じ `verify-virtual` を B 単独でも実行した。

| 旧版・B 検証記録 | 結果 |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline`、復元直後 | 終了101。nonce 競合試験の scoped thread が owner を借用していた。参照の保持を直した。 |
| `cargo test -p splitcast-rs-core --locked --offline --test two_agents --test control_integration` | 終了0。21 と18試験を実行。 |
| `verify`、`6e9e513`、xtask `1795156803822438231` | 8/10。遅延 Prepare の試験が旧所有者を使い、制作側が資源を確保する前に拒否した。 |
| `verify`、`b7f3c8e`、xtask `1795157167319030416` | 9/10。no-default-features の通常 Start で I/O 期限超過。 |
| `verify-virtual`、`b7f3c8e`、xtask `1795157577582007034` | 14/14。 |
| `verify` 再試行、同じ `b7f3c8e`、xtask `1795157847325765196` | 10/10。 |

旧 Agent バイナリは A で
`cargo build -p splitcast-rs-core --locked --offline` を隔離環境で実行したもの。
SHA-256 は `6c4f78d8ee24a338105f9b1caf5dd22df41e072bc95f520dedd2ae5460f1fc08`。
`SPLITCAST_TEST_V2_AGENT` にその生成物を指定して、同じ隔離環境で
`cargo test -p splitcast-rs-core --locked --offline --test two_agents paired_v2_v3_mismatches_refuse_both_directions_before_production_effects -- --exact --ignored --nocapture`
を実行し終了0。v2 がどちらの役でも、メディア有無を含む4ケースで制作側
Idle と子プロセスなし、キャプチャ側の解放責務なしを観測した。
旧 Prepare は同じ制作側の後続セッションと再起動後の双方で拒否。
競合する2つの nonce は1件だけが取得し、同じ所有者と用途の再問い合わせは成功。
部分送信、期限切れ、応答喪失、別所有者や異なる Stop 番号の拒否を
ソケット・状態遷移と実 Agent の試験で確認した。

## C · 観測値と観測 I/O

元の C は A を親とする `1ad678ae574419941a5f17248339e25f6d51e647`。
統合は `25d3def5a72c7d167518aba27186c25137414b56`。
単独の `verify`（xtask `1795154732438257669`）は10/10、
`verify-virtual`（`1795154969616521284`）は14/14。両方の receipt は
同一のクリーンな C を指す。A の保持編集試験はこの段階では同じ誤停止を再現した。

単独 C は `nix develop --offline --no-write-lock-file .#media-test -c` の後に
`bwrap --unshare-all --die-with-parent --ro-bind / / --bind "$PWD" "$PWD" --dev /dev --proc /proc --tmpfs /tmp --tmpfs /run/user --setenv TMPDIR /tmp`
と `--unsetenv` の `DISPLAY`、`WAYLAND_DISPLAY`、`SSH_AUTH_SOCK`、
`DBUS_SESSION_BUS_ADDRESS`、`PULSE_SERVER`、`PULSE_RUNTIME_PATH`、
`PIPEWIRE_REMOTE`、`PIPEWIRE_RUNTIME_DIR` を組み合わせた。
後続 argv は `cargo run -p splitcast-xtask -- verify` または `verify-virtual`。
保持編集は
`cargo test --locked --offline -p splitcast-rs-core --test two_agents production_held_scene_edit -- --exact --nocapture`。
convergence `architecture-convergence-unit-c` の
`attempt-000001`～`attempt-000004` に隔離、通常、仮想、保持編集を記録した。

## D · 制作側監視の所有とキャンセル

実装 `2805a123a900cb70d516ac0770ce7ce69c82e13d`。
統合で判明した HTTP 応答と汎用障害時の Stop の回帰を
`2e06eea0a7d5a3c46c2a85166efb1bed76883603` で修正した。
監視は制作側 Running に1本、1回に1サイクル、更新は最新の1件。
MediaMTX GET の上限は2秒、OBS の認証を含む状態読取は5秒、次の
サンプルは完了から約1秒後。8秒経過したスナップショットは失効。
所有者を切り替えずに Start の比較状態をスレッドへ移し、状態変更は Agent が行う。
故障で通常操作を取り消し、明示 Stop の受付を優先し、監視を join してから
独立の15秒の後始末を開始する。
[監視契約](../virtual-control-design.md#production-running-supervision)

最初の D 試行 `d-targeted.31MsuAwt.log` は終了101。編集は両腕5回通ったが、
全 OBS 接続停止時とストリームカウンター異常時の Status が待ち行列で期限超過した。
後始末中の正規のメタデータ観測を実装し、資源を未観測として返すよう直した。
`d-obs-faults.*.log` の再試行は5試験すべて成功。さらに最初の統合では
このメタデータ再確認が完了済み応答を待たせ、汎用障害で世代を進める変更が
誤った Stop の試験を失敗させた。再確認を非ブロッキングにし、障害による世代失効を監視障害に限定した。
最終修正 `88156db` は Status の受付前に正規の Stop メタデータを読む。
待ち行列に入った後の再確認は非ブロッキングで、完了済み応答の送信が優先される。


最終 SHA の対象制御は終了0（`candidate-controls.oqfKqRyS.log`）。

| 観測 | 最終 SHA の結果 |
| --- | --- |
| 100 ms 編集 | 5/5 適用、全回 Running のまま次の進捗を2回確認。保持中の読取0回。 |
| 3,500 ms 編集 | 5/5 適用、全回 Running のまま次の進捗を2回確認。保持中の実測は各回 MediaMTX 3回・OBS 3回。 |
| 停止 byte、壊れたパス、OBS カウンター異常 | 各障害で出力停止を検出。期限内に録画または配信テストの停止要求と出力非稼働を確認。 |
| OBS の全接続を保留 | 別の後始末接続を試み、11.663 s で最終 Failed を観測。出力非稼働は未確認として返し、PID とリスナーの消失を別途確認。 |
| 制作側 worker 終了 | 0.692 s で終了に対処し、出力非稼働、observer と PID の消失を観測。 |
| Start の応答を保持した Stop | 3/3 で受付後に所有者と `stop_id` が一致する完了。取消済み Start は Running に移らず、明示的な新規 Start は成功。 |
| 編集中の緊急 Stop | HTTP 202 を61.1 ms で受信。結果が不確実な編集を打ち切り、同じ `stop_id` の完了を観測。 |

`/proc/<agent-pid>/task/*/comm` を見て、制作側 Idle/Ready で0、
Running で1、Stop 後に0を確認した。異常終了、期限を越えた観測、
旧世代からの結果と、明示 Stop 優先は状態遷移テストでも確認した。

## G · 統合検証

最終ソフトウェア検証対象は `88156db27742390d584f0339fe672bbd1666a6c6`、tree
`275ca4a0e7778bca0ffb9851d32e74e7865dd71c`。A と同じ Cargo、Studio、
flake の lock を使った。Rust/Cargo 1.95.0、Nix 2.34.9、x86_64-linux。
生成メディアの隔離ランチャーと、その他の試験の Bubblewrap 隔離は
ネットワーク、PID、デバイスを分けた。Nix はビルドサンドボックスで評価した。

最初の統合 `fea8ad8b725be8996c268f518cdef4963256e9db` の `verify`
（xtask `1795157840178880396`）は8/10。既定／no-default-features の両方で
HTTP 終了待ちと障害後の正式 Stop の試験が失敗した。同じ SHA の
`nix flake check --no-write-lock-file --max-jobs 1 --cores 4` も同じ2試験で終了1。
`verify-media` の `20261120T065700Z-1h9Fab9h` は6/7。元の正常録画は
12.366 s だったが、負の対照に加えた0.8 s の音声シフトが全体長を
13.109 s に伸ばし、A/V 判定前に13 s 上限で拒否された。
`1e303b267c754f776ec12a264c6c7ff7221109ef` は、音声ずれと音声 PTS ギャップの
変異ファイルを元録画の長さで切り、元録画の合格条件と A/V 検出値を保持した。

同じ PID 名前空間内の番号が再利用され、以前のディレクトリが残る
`two-agents-PID-N` の作成が `AlreadyExists` になった。最初の最終
`verify`（`1795158236990041654`）、対象制御と Chromium のログを保全。
再実行では各名前空間に別の記録先を bind した。対象制御の初回
`cargo test` ではフィルターを2個渡して終了1。単一の
`agent::lifecycle_tests::` フィルターで14試験が通った。

`1e303b2` の別記録先の `verify`（`1795158490424601215`）は8/10、
`verify-virtual`（`1795158834383866226`）は12/14。両フィーチャーで
Start 応答を保持した Stop の追跡が失敗した。制作側の同じ所有者と番号の
完了は別接続で観測でき、キャプチャ側は解放責務を保持した。
診断用ワークツリーで失敗理由が `control command not executed` であることを確認。
strace を使った別実行は終了0。診断用ログ付きに最終修正を加えた試行は
失敗した操作を3回とも完了した。同時実行数を1にした `1e303b2` の
`verify`（`1795158842637869947`）は10/10。
別途、複数の Cargo 実行が同じデバッグバイナリを再リンクした対象試験では、
スタンドインの `Permission denied` と `(deleted)` を確認した。
最終実行では別記録先と別ビルド先を使用した。

### 再実行コマンド

```bash
isolated=(bwrap --unshare-all --die-with-parent --bind / / --dev /dev --proc /proc
  --tmpfs /tmp --tmpfs /run/user --setenv TMPDIR /tmp
  --unsetenv DISPLAY --unsetenv WAYLAND_DISPLAY --unsetenv SSH_AUTH_SOCK)
controlled=("${isolated[@]}" --setenv RUST_TEST_THREADS 2 --setenv CARGO_BUILD_JOBS 2)
separate_target=("${controlled[@]}" --setenv CARGO_TARGET_DIR "$PWD/artifacts/target")
mkdir -p artifacts/architecture-handoff

for check in verify verify-virtual; do
  fresh=$(mktemp -d "$PWD/artifacts/architecture-handoff/$check.XXXXXXXX")
  nix develop .#media-test -c "${separate_target[@]}" --bind "$fresh" "$PWD/artifacts" \
    cargo run -p splitcast-xtask -- "$check"
done

CARGO_BUILD_JOBS=2 nix develop .#media-test -c cargo run -p splitcast-xtask -- verify-media
nix flake check --no-write-lock-file --max-jobs 1 --cores 4

fresh=$(mktemp -d "$PWD/artifacts/architecture-handoff/controls.XXXXXXXX")
nix develop .#media-test -c "${separate_target[@]}" --setenv SPLITCAST_REPEAT_HELD_EDIT 1 \
  --bind "$fresh" "$PWD/artifacts" bash -ec '
    cargo test -p splitcast-rs-core --test two_agents --locked --offline production_ -- --nocapture
    cargo test -p splitcast-rs-core --test two_agents --locked --offline studio_scene_proxy_preserves_stop_under_withheld_obs_acknowledgment -- --exact --nocapture
  '

fresh=$(mktemp -d "$PWD/artifacts/architecture-handoff/web.XXXXXXXX")
nix develop .#media-test -c nix shell --inputs-from . nixpkgs#nodejs nixpkgs#chromium \
  --no-write-lock-file -c "${controlled[@]}" --unsetenv DBUS_SESSION_BUS_ADDRESS \
  --bind "$fresh" "$PWD/artifacts" cargo test -p splitcast-rs-core --test two_agents \
  --locked --offline return_tests::chromium_speaks_through_the_agent_and_observes_stop \
  -- --exact --ignored --nocapture
```

```bash
bwrap_bin=$(nix develop .#media-test -c sh -c 'command -v bwrap')
studio_isolation=("$bwrap_bin" "${isolated[@]:1}" --setenv CARGO_BUILD_JOBS 2)
nix develop .#studio -c "${studio_isolation[@]}" bash -ec '
  cargo test --manifest-path packages/splitcast-studio/Cargo.toml --locked --offline --release
  cargo build --manifest-path packages/splitcast-studio/Cargo.toml --locked --offline --release
'
studio_bin="$PWD/packages/splitcast-studio/target/release/splitcast-studio"
nix develop .#studio-test -c "${studio_isolation[@]}" --unsetenv DBUS_SESSION_BUS_ADDRESS \
  python3 packages/splitcast-studio/tests/gui.py "$studio_bin"
fresh=$(mktemp -d "$PWD/artifacts/architecture-handoff/wayland.XXXXXXXX")
nix develop .#studio-wayland-test -c "${studio_isolation[@]}" --unsetenv DBUS_SESSION_BUS_ADDRESS \
  python3 packages/splitcast-studio/tests/wayland.py --studio-binary "$studio_bin" \
  --studio-source "$(git rev-parse HEAD)" --evidence "$fresh/evidence"

nix eval --impure --json --no-write-lock-file --expr '
let self = builtins.getFlake (toString ./.);
    homeManager = builtins.getFlake "github:nix-community/home-manager/5643e73425b85ac359ba2aaf83deae8ce1cd7968";
in (import ./nix/tests.nix {inherit homeManager self; nixpkgs = self.inputs.nixpkgs;}).evaluation
'
```

### 結果と個別の証拠

| 確認 | 最終ソフトウェア SHA の実測 |
| --- | --- |
| `verify` 初回 | xtask `1795159469952684737` は9/10。no-default-features の3.5秒編集が HTTP 409 で取消になった。二回目の MediaMTX 読取を確認し、対応する OBS 読取は観測できなかった。原因は未確定。 |
| 通常検証の再試行 | xtask `1795169879753020362`、10/10。別記録先で `RUST_TEST_THREADS=1` を指定し、既定機能と no-default-features 各181試験が成功。convergence `architecture-final-verify-serial` の `attempt-000001`、終了0。 |
| `verify-virtual` | xtask `1795159470399754121`、14/14。既定機能は181試験、private-audio、generated-media、native-return、local-transport も実行した。 |
| `verify-media` | 初回 `20261120T072422Z-jiz1ZTV6` は6/7。worker-death 本体の検出と解放後、新規録画で100 msのマイク欠落が連続5窓あった。同じ SHA の再試行 `20261120T073337Z-b6B4PEYF` は7/7、判定器21試験も成功。初回欠落の原因は未確定。 |
| Studio とネイティブ操作 | Studio リリース6試験、X11、Wayland、CLI/TUI、Chromium が成功。Wayland の非フォーカス chat、保存したレイアウトの再読取、操作遅延、所有者に対応する Stop を確認した。 |
| Nix | `nix flake check` は2件とも成功。Home Manager と NixOS の評価23件が成功。パッケージの selfcheck と両ロールの TOML 検証も終了0。 |
| Python | `nix shell --inputs-from . nixpkgs#python3Packages.pyflakes --no-write-lock-file -c pyflakes tools/probes/unit_e_obs_pipe.py tools/media_acceptance.py` は終了0。 |

生成メディアの再試行は Agent の SHA-256
`5939bf0fe4f9fc444f42c31d4f541e9bf523a5a4b2e942e61340751d1f3cb723`。
標準の11.900 s とカメラ付きの11.933 sの録画を独立してデコードした。
両方で OBS 出力非稼働、OBS 終了0、プロセスとリスナーの解放を確認した。
worker-death では2.211 s以内に Failed を観測し、11.800 sの別録画を
デコードした。停止や遅延を注入した試験と、進捗が続いていても
凍結・滞留した映像を拒否する判定器を含む。
ソフトウェア描画の遅延フレームは標準0.8%、カメラ付き3.8%。
[生成メディア基準](../media-acceptance.md)

Studio バイナリの SHA-256 は
`d3604c27abe043363e50c83cd5a327322622ed4562e221938c533250d6e6d1d7`。
Chromium 151.0.7922.173 と Node 24.19.0 を使用した。
Nix パッケージの outPath は `vgr2wbxhl3msgljk97qrwfyz6wn3brrs-splitcast-rs-core-0.1.0`。
`nix build --no-link --print-out-paths --no-write-lock-file .#default` の出力を
`pkg` として `"$pkg/bin/splitcast" selfcheck`、
`"$pkg/bin/splitcast" validate --config packages/splitcast-rs/example.toml`、
同じ `example-production.toml` を実行した。

最終結果と失敗ログの控えは `artifacts/architecture-handoff/88156db/`。
その他のローカル証拠は `artifacts/architecture-handoff/` と、Git worktree の管理ディレクトリ
内の `handoff/`。対象制御は `candidate-controls.oqfKqRyS.log`、Studio と
Chromium は `candidate-clients.XOwl6F5z/`。通常と仮想の最終コード用
convergence は `architecture-final-verify` と `architecture-final-verify-virtual`。
仮想の `attempt-000001` に、実行 argv、入力スナップショット、終了0を記録した。
実装コードの検証後にこの記録を別コミットで追加する。

## 変更量と切戻し

- 取得の一時所有者スロットを廃止した。Preparing の所有者への確定と、
  解放責務の型付き照合で、Stop 後の再取得を抑止する。ペアの互換性確認は
  既存の routes を使用し、セッション履歴を保持しない。
- Agent の監視時刻と比較器、OBS プロファイルの進捗履歴を取り除いた。
  受信 JSON は1応答に1回デコードする。制作側 Running の joined observer
  1本と最新スナップショット1件が比較と配信を担う。
- A から最終ソフトウェア SHA まで、Rust の `src/` は試験込みで
  +2,090／−720行、Rust の `tests/` は +828／−141行。依存追加は0。
  制作マイクのプロセス数は専用 PulseAudio を含む4。E の比較器は試験専用。
- D の切戻し先は B/C が揃った `2a9b077aa41bf0bd93272b0feec866e7d1616b74`。
  B を切り戻す場合は所有資源の解放を確認し、両役を揃えて
  `31901d10e9f13c16c9bdcb147dfd74dfcf28f014` に戻す。
  v2 と v3 の混用は双方の方向で拒否する。

## 運用ゲート

`rustCandidate=false`、空の起動ターゲット、両ロールの設定例を
既存どおり評価した。実機のゲームフレーム時間 p99、入力遅延、Direct Scanout と
ローカルゲーム音の同時確認、NVIDIA 制作側の描画・エンコード、物理 A/V とカメラ時差、
配信中の Twitch/VOICEVOX 返送、実サービスの cgroup と視聴側の解放、
実際の NixOS/Home Manager 構成の起動は今回未検証。
Miri、fuzzing、依存関係のオンライン監査も未実施。
[現行ゲートと以前の物理記録](../memory/session-state.md)
