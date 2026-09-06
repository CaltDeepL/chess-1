import { useNavigate } from "react-router-dom";
import type { GameResult } from "../types";

interface GameOverOverlayProps {
  result: GameResult;
  myColor: "white" | "black" | null;
}

const RESULT_LABEL: Record<string, string> = {
  white_win: "白の勝ち",
  black_win: "黒の勝ち",
  draw: "引き分け",
};

export default function GameOverOverlay({ result, myColor }: GameOverOverlayProps) {
  const navigate = useNavigate();
  if (!result) return null;

  const didWin =
    (result === "white_win" && myColor === "white") ||
    (result === "black_win" && myColor === "black");
  const didLose =
    (result === "white_win" && myColor === "black") ||
    (result === "black_win" && myColor === "white");

  const heading = didWin ? "WIN" : didLose ? "LOSE" : RESULT_LABEL[result] ?? "対局終了";

  return (
    <div className="game-over-overlay">
      <div className="game-over-card">
        <h2 className={didWin ? "result-win" : didLose ? "result-lose" : ""}>{heading}</h2>
        <p>{RESULT_LABEL[result] ?? result}</p>
        <div className="game-over-actions">
          <button onClick={() => navigate("/lobby")}>ロビーへ戻る</button>
        </div>
      </div>
    </div>
  );
}