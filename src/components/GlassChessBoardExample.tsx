import { Chessboard } from "react-chessboard";
import { unicodeGlassPieces } from "./UnicodePieces";
import "./glass-board.css";

// react-chessboard v5 (options API) を想定した組み込み例。
// 盤面は .glass-board-frame 側の aspect-ratio + min(92vw, 92vh, 720px) で
// 画面サイズに応じて正方形を保ったまま伸縮する。
// react-chessboard 自体には boardStyle で 100% を渡し、親要素いっぱいに広げる。

const lightSquareStyle = {
  background: "linear-gradient(135deg, rgba(210,230,255,0.55), rgba(180,210,245,0.25))",
  backdropFilter: "blur(6px)",
};

const darkSquareStyle = {
  background: "linear-gradient(135deg, rgba(30,55,110,0.75), rgba(10,20,50,0.55))",
  backdropFilter: "blur(6px)",
};

// 全マス共通(単数形)のスタイル。
// - overflow: hidden … 駒のグローがマス境界を越えて滲むのを防ぐ
// - containerType: "inline-size" … このマスを「コンテナ」化し、
//   中の .glass-piece が cqw 単位でマス自身の実寸に追従できるようにする
const squareStyle: React.CSSProperties = {
  overflow: "hidden",
  // containerType は現時点の React.CSSProperties 型に含まれないため any 経由で指定
  ...({ containerType: "inline-size" } as Record<string, string>),
};

export function GlassChessBoardExample({
  fen,
  onPieceDrop,
}: {
  fen: string;
  onPieceDrop: (sourceSquare: string, targetSquare: string) => boolean;
}) {
  return (
    <div className="glass-board-frame">
      <div className="glass-board-inner">
        <Chessboard
          options={{
            position: fen,
            pieces: unicodeGlassPieces,
            lightSquareStyle,
            darkSquareStyle,
            squareStyle,
            onPieceDrop: ({ sourceSquare, targetSquare }) =>
              onPieceDrop(sourceSquare, targetSquare ?? ""),
            boardStyle: { width: "100%", height: "100%", borderRadius: 6 },
          }}
        />
      </div>
    </div>
  );
}

/*
補足: もし使用中の react-chessboard のバージョンが boardStyle の width/height:100%
だけでは追従せず、数値の boardWidth(px)を明示的に要求するタイプであれば、
親要素(.glass-board-inner)を ResizeObserver で監視し、
実測幅を options.boardWidth に渡す形に変更する。例:

import { useEffect, useRef, useState } from "react";

function useElementWidth<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    if (!ref.current) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(entry.contentRect.width);
    });
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}
*/
