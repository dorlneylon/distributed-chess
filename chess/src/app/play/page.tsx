'use client';

import React, { useState, useEffect } from "react";
import { Card } from "@nextui-org/react";
import { GameState, Color } from "../../pb/game";
import { useRouter, useSearchParams } from "next/navigation";
import { createChannel, createClient } from "nice-grpc-web";
import Image from 'next/image';
import { NodeDefinition, Position } from "@/pb/query";
import cx from 'classnames';
import { motion } from 'framer-motion';

const pieceToSvg: Record<string, string> = {
    "r": "/assets/rook-b.svg",
    "n": "/assets/knight-b.svg",
    "b": "/assets/bishop-b.svg",
    "q": "/assets/queen-b.svg",
    "k": "/assets/king-b.svg",
    "p": "/assets/pawn-b.svg",
    "R": "/assets/rook-w.svg",
    "N": "/assets/knight-w.svg",
    "B": "/assets/bishop-w.svg",
    "Q": "/assets/queen-w.svg",
    "K": "/assets/king-w.svg",
    "P": "/assets/pawn-w.svg",
};

export default function Play() {
    const [gameState, setGameState] = useState<GameState>({} as GameState);
    const [selectedCell, setSelectedCell] = useState<Position | null>(null);
    const [isBoardReversed, setIsBoardReversed] = useState(false);
    const [piecePositions, setPiecePositions] = useState<Map<string, { x: number, y: number }>>(new Map());

    const sessionUser = sessionStorage.getItem('username') || '';
    const addr = sessionStorage.getItem('addr') || '';
    const whitePlayer = useSearchParams().get('white_player') || '';
    const blackPlayer = useSearchParams().get('black_player') || '';
    const router = useRouter();

    const channel = createChannel(`http://${addr}`);
    const client = createClient(NodeDefinition, channel);

    useEffect(() => {
        const fetchGameState = async () => {
            try {
                const response = await client.state({ whitePlayer, blackPlayer });

                if (response.state) {
                    setGameState(response.state);
                }
            } catch (e) {
                console.error('Error fetching game state:', e);
            }
        }

        const intervalId = setInterval(fetchGameState, 500);
        return () => clearInterval(intervalId);
    }, [client, whitePlayer, blackPlayer]);

    useEffect(() => {
        setIsBoardReversed(sessionUser === gameState.whitePlayer);
    }, [gameState, sessionUser]);

    useEffect(() => {
        if (gameState.board) {
            const newPiecePositions = new Map<string, { x: number, y: number }>();
            gameState.board.rows.forEach((row, rowIndex) => {
                row.cells.forEach((cell, colIndex) => {
                    if (cell.piece) {
                        const pieceKey = `${cell.piece.color}${cell.piece.kind}${rowIndex}${colIndex}`;
                        newPiecePositions.set(pieceKey, { x: rowIndex, y: colIndex });
                    }
                });
            });
            setPiecePositions(newPiecePositions);
        }
    }, [gameState]);

    const handleCellClick = async (pos: Position) => {
        if (selectedCell) {
            const actualFromPos = isBoardReversed
                ? { x: 7 - selectedCell.x, y: selectedCell.y }
                : selectedCell;
            const actualToPos = isBoardReversed
                ? { x: 7 - pos.x, y: pos.y }
                : pos;

            await makeMove(actualFromPos, actualToPos);
            setSelectedCell(null);

            try {
                const response = await client.transact({
                    whitePlayer,
                    blackPlayer,
                    action: [
                        actualFromPos,
                        actualToPos,
                    ]
                });
            } catch (e) {
                console.error('Error making move:', e);
            }
        } else {
            setSelectedCell(pos);
        }
    };

    const makeMove = async (from: Position, to: Position) => {
        const newBoard = JSON.parse(JSON.stringify(gameState.board));
        const piece = newBoard.rows[from.x].cells[from.y].piece;

        if (piece) {
            newBoard.rows[to.x].cells[to.y].piece = piece;
            newBoard.rows[from.x].cells[from.y].piece = null;

            const pieceKey = `${piece.color}${piece.kind}${from.y}${from.x}`;
            setPiecePositions(prev => new Map(prev.set(pieceKey, { x: to.x, y: to.y })));
            setGameState({ ...gameState, board: newBoard });
        }
    };

    const getFigSrc = (row: number, col: number): string => {
        const actualRow = isBoardReversed ? 7 - row : row;
        const actualCol = isBoardReversed ? col : col;
        const fig = gameState.board?.rows[actualRow].cells[actualCol].piece;
        if (!fig) return '';
        return fig.color === Color.WHITE ? pieceToSvg[fig.kind.toUpperCase()] : pieceToSvg[fig.kind.toLowerCase()];
    }

    return (
        <main className="flex flex-col items-center justify-center min-h-screen bg-zinc-900">
            <Card className="p-10 bg-zinc-950 shadow-lg rounded-lg max-w-xl w-full">
                <h1 className="text-3xl font-semibold text-center mb-6">Playing with</h1>
                <ul className="text-sm -mt-3 font-semibold text-center mb-6">{sessionUser === whitePlayer ? blackPlayer : whitePlayer}</ul>
                <div className="grid grid-cols-8 gap-0 relative">
                    {gameState.board?.rows.map((row, rowIndex) => (
                        row.cells.map((_, colIndex) => {
                            const pieceSrc = getFigSrc(rowIndex, colIndex);
                            const pieceKey = `${gameState.board?.rows[rowIndex].cells[colIndex].piece?.color}${gameState.board?.rows[rowIndex].cells[colIndex].piece?.kind}${rowIndex}${colIndex}`;
                            return (
                                <div
                                    key={`${rowIndex}-${colIndex}`}
                                    onClick={() => handleCellClick({ x: rowIndex, y: colIndex })}
                                    className={cx("w-18 h-16 flex items-center justify-center",
                                        selectedCell?.x === rowIndex && selectedCell?.y === colIndex ? "border-2 border-blue-500" : "",
                                        (rowIndex + colIndex) % 2 === 0 ? "bg-gray-300" : "bg-gray-600"
                                    )}
                                >
                                    {pieceSrc && (
                                        <motion.div
                                            layoutId={pieceKey}
                                            initial={{ opacity: 0 }}
                                            animate={{ opacity: 1 }}
                                            transition={{ duration: 0.3 }}
                                            style={{ position: 'absolute' }}
                                        >
                                            <Image
                                                src={pieceSrc}
                                                alt=''
                                                width={50}
                                                height={50}
                                            />
                                        </motion.div>
                                    )}
                                </div>
                            );
                        })
                    ))}
                </div>
            </Card>
        </main>
    );
}
