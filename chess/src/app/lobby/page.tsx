'use client';

import React, { useState, useEffect } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { Input, Button, Spacer, Card, CardBody } from '@nextui-org/react';
import { createChannel, createClient } from 'nice-grpc-web';
import { NodeDefinition } from '../../pb/query';

export default function Lobby() {
    const [opponent, setOpponent] = useState<string>('');
    const router = useRouter();

    const searchParams = useSearchParams();
    const addr = searchParams.get('addr');
    sessionStorage.setItem('addr', addr!);

    const publicKeyString = sessionStorage.getItem('publicKey')!;

    const channel = createChannel(`http://${addr}`);
    const client = createClient(NodeDefinition, channel);


    useEffect(() => {
        if (!addr) return;

        const checkForInvitation = async () => {
            try {
                const response = await client.isInGame({ player: publicKeyString });

                if (response.state) {
                    router.push(`/play?white_player=${response.state!.whitePlayer}&black_player=${response.state!.blackPlayer}`);
                }
            } catch (e) {
                console.error('Error checking for invitation:', e);
            }
        };

        const intervalId = setInterval(checkForInvitation, 5000);

        return () => clearInterval(intervalId);
    }, [addr, publicKeyString]);

    const handleStartGame = async () => {
        try {
            const response = await client.start({ whitePlayer: publicKeyString, blackPlayer: opponent });
            console.log(response);

            if (response.state) {
                router.push(`/play?white_player=${response.state.whitePlayer}&black_player=${response.state.blackPlayer}`);
            }
        } catch (error) {
            console.error('Error starting game:', error);
        }
    };

    return (
        <main className="flex flex-col items-center justify-center min-h-screen bg-zinc-900">
            <Card className="p-10 bg-zinc-950 shadow-lg rounded-lg max-w-md w-full">
                <h1 className="text-3xl font-semibold text-center mb-6">Lobby</h1>
                <h4 className="text-xl font-semibold text-center mb-6">Your public key: </h4>
                <p className='text-[10px] text-center indent-0 mr-2 pb-5 mr-4 -mt-2 -ml-2'>{publicKeyString}</p>
                <Card className="mb-6 bg-zinc-900">
                    <CardBody className="text-center text-white w-full">
                        Enter the public key of your opponent or wait for an invitation.
                    </CardBody>
                </Card>
                <Input
                    isClearable
                    placeholder="Opponent's Public Key"
                    value={opponent}
                    onChange={(e) => setOpponent(e.target.value)}
                    className="mb-4"
                />
                <Spacer y={1} />
                <Button onClick={handleStartGame} className="w-full bg-zinc-800">
                    Start a Game
                </Button>
            </Card>
        </main>
    );
}
