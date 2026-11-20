# 制作マイクの OBS パイプ採用判定

専用 PulseAudio を維持する。パッケージ版 OBS は無音を含むパイプの
PCM を読み取ったが、短時間の同期比較で全3マーカーが採用基準を超えた。
設計の F は保留し、B と D の実装を進めた。
[設計と採用基準](architecture-convergence-20261120.md#6-acceptance-and-rollback-contracts)

## 試験条件と再現

設計 `f089da89995da9d8ce2ba515a01eb814a68c721d`、親
`ca213f8cc9b5009b603528042fea0a6eb2b271b0`。今回の同期試験起動時の
実装ブランチは A の `31901d10e9f13c16c9bdcb147dfd74dfcf28f014` で、
B の復元パッチが作業中だった。試験は Agent を起動せず、両方式で同じ
固定済みのパッケージと生成入力を使用した。
`flake.lock` の Git blob は `026daf957b056470b8a1c991bdca5dd41129c63a`。
試験器の Git blob は `6f92392500e127201dadefa20665b1a92ef2e4d7`。
[再現用プローブ](../../tools/probes/unit_e_obs_pipe.py)

```sh
nix develop --no-write-lock-file .#media-test -c python3 tools/probes/unit_e_obs_pipe.py --timing
```

パッケージ版 OBS 32.1.2、OBS WebSocket 5.7.3、obs-ffmpeg の実ロード先
`libavformat.so.62.12.102`、コマンドライン FFmpeg Full 8.1.2、RNNoise LADSPA
1.10。生ログにはパッケージのラッパー、ロード済み共有ライブラリ、
コマンドライン FFmpeg、RNNoise と strace の個別 SHA-256 がある。各方式は
別の Bubblewrap ネットワーク・PID 名前空間で実行した。ループバックのみ、
物理メディアデバイスのない `/dev`、Xvfb、ソフトウェア描画を確認した。
実行記録は `artifacts/unit-e-obs-pcm/runs/attempt-_jnh7yo8/`。
集約ログ `artifacts/architecture-handoff/e-timing.cYsOkqmh.log` の
SHA-256 は `7d9fe025a2572bb6900146267b2881e9be91142db4b3cbf8efa66a651970dc0d`。

## 観測結果

両方式で同じ有限 FFmpeg ジェネレーターからゲーム音とマイク音を同期生成した。
入力マーカーは共通タイムラインの20、30、40秒。OBS の同期オフセット設定は
両方式とも 0 ms。独立した FFmpeg の全ストリームデコードと、ffprobe の
トラック別 PTS および PCM の周波数・継続幅から測定した。
正値はマイクの遅れ、負値は先行を表す。

| 入力マーカー | Pulse のマイク対ゲーム差 | パイプのマイク対ゲーム差 | パイプの絶対誤差の下限 |
| --- | ---: | ---: | ---: |
| 20 s | −60 ms | −1,220 ms | 1,199 ms |
| 30 s | −70 ms | −1,270 ms | 1,249 ms |
| 40 s | −70 ms | −1,270 ms | 1,249 ms |

各測定の不確かさは約 ±21.34 ms。パイプの絶対誤差と Pulse に対する悪化は
下限でもそれぞれ 500 ms、100 ms を超えた。方式別の実行は両方終了0。
比較コマンド全体は終了1。原因と長期ドリフトは未確定。

- OBS の fd 0 と RNNoise の stdout は同じ inode の 8,192 B パイプだった。
  無音入力中、接続前 7,680 B が `PLAYING` で 0 B に減り、writer は生存した。
- 引き継いだ短時間の連続音試験 `attempt-xyg0uk3i` は両方式とも終了0。
  マイクの識別マーカーはミックスとマイク専用トラックの99窓すべてにあり、
  ゲーム専用トラックの99窓では0。カメラなしで実行した。
- 同期試験の両方式で StopRecord 後の出力非稼働、OBS 終了0、プロセスグループ解放、
  リスナー消失を確認した。追跡対象の囮ソケットへの OBS 接続は両方式とも0。
- 先行試行の早すぎるシーン問い合わせ、Xvfb の起動失敗、モノラルの指定不備、
  周波数検出の誤一致、録画開始で欠けた最初のマーカーを元の作業ツリーに保全した。
  同期の再試行ではマーカーを録画開始より後に配置して両方式を完走した。

同期の前提試験が不合格のため、10分の比較、カメラ付き試験、ミュート・同期設定の
変更、音声返送との分離、断線・非読取・FD残留・強制解放の採用マトリクスは未実施。
短時間試験の終了とリソース観測は、これらの検証を代替しない。
