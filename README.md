# Eyecatch Studio

ロゴやアイコンを中央に配置した、16:9のアイキャッチ画像を作る小さなRust / eguiアプリです。
ホームページやCMSとは接続しない独立したプロジェクトです。

## 起動

```sh
mise install
mise run run
```

Rust 1.100.0とLinuxのデスクトップ環境で動作確認しています。Rustがインストール済みなら `cargo run --release --locked` でも起動できます。

1. 左の一覧から素材を選ぶ。名前・分野で検索できます。
2. 背景色とマークの大きさを調整する。汎用アイコンは線の色も変更できます。
3. 「設定を保存」で `catalog.json` に保存する。
4. 「この画像を出力」または「すべて出力」でPNGを書き出す。

SVG・PNG・JPEG・WebPのファイルをドロップするか、パスを入力して素材を追加できます。透明な余白は自動で取り除き、縦横比を保って中央に配置します。白背景のラスター画像は背景もそのまま残るため、透過素材を使うと背景色を活かせます。

出力は **1600 × 900 px**。標準の保存先はこのプロジェクトの `output/` です。同名ファイルは上書きします。編集中の設定で出力できますが、次回起動にも残すには「設定を保存」を押してください。

GUIと同じ描画処理で一括出力もできます。

```sh
mise run export
cargo run --release --locked -- --export-all /path/to/output
```

全件出力時は画像名・技術名・背景色・素材出典を含む `manifest.json` も作成します。画像はメディアライブラリへ手動で登録できます。自動アップロード機能はありません。

## 素材と設定

- `assets/`: ダウンロード済みのSVGとライセンス。オフラインで使用可能。
- `catalog.json`: 各画像の名前、素材、背景、サイズ、汎用アイコン色。
- `assets/sources.json`: 素材の取得元コミット。
- `scripts/fetch_assets.py`: 固定コミットから素材を再取得。既存の素材・設定は上書きしません。
- `src/render.rs`: SVG読み込み、余白除去、合成、PNG出力。
- `src/main.rs`: egui画面と一括出力コマンド。

同梱プリセットは技術30種類と汎用アイコン24種類×2配色の78枚です。

技術マークは主にDeviconの `original-wordmark` を優先し、なければ `original` を採用しています。追加の技術名は描画せず、元の色を保持します。汎用アイコンはLucideを使用しています。各商標はそれぞれの権利者に帰属します。素材の出典は各プリセットの `source` に記録しています。

- KiCad・PlatformIOは各プロジェクトの公式リポジトリから取得。出典は `catalog.json` に記録。
- InventorはAutodeskを出典とするWikimedia CommonsのSVG、OnshapeはPTC公式ブランド素材の白抜き版を使用。ZIP内の素材は出典URLのフラグメントに格納先を記録。
- [Devicon](https://github.com/devicons/devicon) — `assets/DEVICON-LICENSE`
- [Lucide](https://github.com/lucide-icons/lucide) — `assets/LUCIDE-LICENSE`
- [eframe](https://docs.rs/eframe/0.37.0/eframe/)
- [resvg](https://docs.rs/resvg/0.50.0/resvg/)

Gitではソース、ロックファイル、素材、設定を管理します。再生成できる `output/` と `target/` は除外しています。

## 検証

```sh
mise run check
cargo build --locked
```
