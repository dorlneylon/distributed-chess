'use client';

import React, { useState } from 'react';
import { useRouter } from 'next/navigation';
import { Input, Button, Spacer, Card, CardBody } from '@nextui-org/react';
import * as secp256k1 from '@noble/secp256k1';

export default function Home() {
  const [addr, setAddr] = useState<string>('');
  const router = useRouter();

  const handleNextPage = () => {
    const privateKey = secp256k1.utils.randomPrivateKey();
    const publicKey = secp256k1.getPublicKey(privateKey);
    sessionStorage.setItem('privateKey', Buffer.from(privateKey).toString('hex'));
    sessionStorage.setItem('publicKey', Buffer.from(publicKey).toString('hex'));
    router.push(`/lobby?addr=${addr}`);
  };

  return (
    <main className="flex flex-col items-center justify-center min-h-screen bg-zinc-900">
      <Card className="p-10 bg-zinc-950 shadow-lg rounded-lg max-w-md w-full">
        <h1 className="text-3xl font-semibold text-center mb-6">Enter Peer Info</h1>
        <Card className="mb-6 bg-zinc-800">
          <CardBody className="text-center text-white w-full">
            Enter the host of the peer you want to connect to,
            and your nickname.
          </CardBody>
        </Card>
        <Input
          isClearable
          placeholder="127.0.0.1:3000"
          value={addr}
          onChange={(e) => setAddr(e.target.value)}
          className="mb-4"
        />
        <Spacer y={1} />
        <Button onClick={handleNextPage} className="w-full bg-zinc-800">
          Connect
        </Button>
      </Card>
    </main>
  );
}
