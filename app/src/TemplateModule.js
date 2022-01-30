import React, { useEffect, useState } from 'react'
import { Form, Input, Grid, Card, List, Modal, Button, Divider } from 'semantic-ui-react'

import { useSubstrateState } from './substrate-lib'
import { TxButton } from './substrate-lib/components'

const BLOCK_RESERVATION_COST = 10;
const convertToHash = entry =>
  `0x${entry.toJSON().slice(-64)}`;

function ucFirst(word) {
  return word.charAt(0).toUpperCase() + word.toLowerCase().slice(1);
}

const constructNameEntry = (hash, {name, owner, expiresAt}) => ({
  id: hash,
  name: String.fromCharCode(...name),
  owner: convertToHash(owner),
  expiresAt: expiresAt.toString(),
});

function Main (props) {
  const { api, currentAccount } = useSubstrateState();

  const [nameEntryHashes, setNameEntryHashes] = useState([]);
  const [nameEntries, setNameEntries] = useState([]);
  const [open, setOpen] = useState(false);

  const [blockNumber, setBlockNumber] = useState(0)

  const bestNumber = true
    ? api.derive.chain.bestNumberFinalized
    : api.derive.chain.bestNumber

  useEffect(() => {
    let unsubscribeAll = null

    bestNumber(number => {
      setBlockNumber(number.toNumber())
    })
      .then(unsub => {
        unsubscribeAll = unsub
      })
      .catch(console.error)

    return () => unsubscribeAll && unsubscribeAll()
  }, [bestNumber])


  const subscribeOwnedNameEntriesHashes = () => {
    let unsub = null;

    const asyncFetch = async () => {
      if (!currentAccount) {
        return
      }
      unsub = await api.query.templateModule.nameEntriesOwned(currentAccount.address, async owned => {
        // Fetch all nameEntry keys
        const hashes = owned.map(convertToHash);
        setNameEntryHashes(hashes);
      });
    };

    asyncFetch();

    return () => {
      unsub && unsub();
      unsub = null;
    };
  };

  const subscribeNameEntries = () => {
    let unsub = null;

    const asyncFetch = async () => {
      unsub = await api.query.templateModule.nameEntries.multi(nameEntryHashes, nameEntries => {
        const nameEntryArr = nameEntries
          .map((nameEntry, ind) => constructNameEntry(nameEntryHashes[ind], nameEntry.value));
        setNameEntries(nameEntryArr);
      });
    };

    asyncFetch();

    // return the unsubscription cleanup function
    return () => {
      unsub && unsub();
      unsub = null;
    };
  };

  useEffect(subscribeNameEntries, [api, nameEntryHashes]);
  useEffect(subscribeOwnedNameEntriesHashes, [api, currentAccount]);

  return (<>
    <Modal
      onClose={() => setOpen(null)}
      open={!!open}
    >
      <Modal.Header>{open ? ucFirst(open.callable) : ''}</Modal.Header>
      <Modal.Content >
        {
          open && <NameEntryForm
          {...open}
        />
        }
      </Modal.Content>
    </Modal>


    <Grid.Column width={8}>
      <h1>Name registry</h1>
      <Card centered fluid>
        <Card.Content>
        <Card.Header>
            My registred names
          <Button size="mini" floated='right'  onClick={() => setOpen({
            callable: 'register',
            initialExpiryBlock: blockNumber,
            blockNumber,
          })}>Register</Button>
        </Card.Header>

        <Divider/>

        <List divided relaxed>
          {nameEntries.map(item => 
            {
              const isRenewable = item.expiresAt >= blockNumber;
              const isCancelable = item.expiresAt > blockNumber;
              const isExpired = item.expiresAt < blockNumber;
            
              return <List.Item key={item.id}>
                {/* <List.Icon name='github' size='large' verticalAlign='middle' /> */}
                {
                  isRenewable && <List.Content floated='right'>
                    <Button size='mini' onClick={(() => setOpen({
                      callable: 'renew',
                      item,
                      initialExpiryBlock: Math.max(blockNumber, item.expiresAt),
                      blockNumber,
                    }))}>Renew</Button>
                  </List.Content>
                }
                {
                  isCancelable && <List.Content floated='right'>
                    <Button size='mini' onClick={() => setOpen({
                      callable: 'cancel',
                      item,
                      initialExpiryBlock: item.expiresAt,
                      blockNumber,
                    })}>Cancel</Button>
                  </List.Content>
                }
                {
                  isExpired && <List.Content floated='right'>
                    <Button size='mini' disabled>Expired</Button>
                  </List.Content> 
                }

                <List.Content>
                  <List.Header as='a'>{item.name}</List.Header>
                  <List.Description as='a'>expires at {item.expiresAt}</List.Description>
                </List.Content>
              </List.Item>
          })}
          </List>
        </Card.Content>
      </Card>

    </Grid.Column>
    </>
  )
}

function NameEntryForm(props) {
  // The transaction submission status
  const [status, setStatus] = useState('')

  const {blockNumber, initialExpiryBlock, callable} = props
  const [formState, setFormState] = useState({ 
    name: (props.item ? props.item.name : '') || '',
    targetExpiryBlock: initialExpiryBlock,
    numBlocks: callable === 'cancel' 
      ? (initialExpiryBlock - blockNumber) 
      : 0,
  })

  const onChange = (_, data) =>
    setFormState(prev => {
      const updated = ({ ...prev, [data.state]: data.value });

      updated.numBlocks = callable === 'cancel'
        ? (initialExpiryBlock - blockNumber)
        : (updated.targetExpiryBlock - Math.max(blockNumber, initialExpiryBlock))

      return updated;
    })

  const { name, numBlocks, targetExpiryBlock } = formState;

  return <Form>
    <Form.Field>
      <Input
        fluid
        label="Name"
        type="text"
        placeholder="name"
        value={name}
        state="name"
        onChange={callable === 'register' ? onChange : () => {}}
      />
    </Form.Field>

    {
      callable !== 'cancel' && <Form.Field>
        <Input
          fluid
          label="Expiry block"
          type="number"
          placeholder="100"
          value={targetExpiryBlock}
          state="targetExpiryBlock"
          min={Math.max(blockNumber, initialExpiryBlock)}
          onChange={onChange}
        />
      </Form.Field>
    }

    <Form.Field>
      {callable === 'cancel' ? 'Refund' : 'Pay'} {numBlocks*BLOCK_RESERVATION_COST} <span className='has-text-weight-bold'>NBT</span> for {numBlocks} blocks
    </Form.Field>
    
    <Form.Field style={{ textAlign: 'center' }}>
      <TxButton
        label={ucFirst(callable)}
        type="SIGNED-TX"
        setStatus={setStatus}
        disabled={!!name && callable !== 'cancel' && numBlocks <= 0}
        attrs={{
          palletRpc: 'templateModule',
          callable,
          inputParams: [name, numBlocks],
          paramFields: callable === 'cancel' ? [true] : [true, true],
        }}
      />
    </Form.Field>
    <div style={{ overflowWrap: 'break-word' }}>{status}</div>
  </Form>
}

export default function TemplateModule(props) {
  const { api } = useSubstrateState()
  return api.query.templateModule && api.query.templateModule.nameEntriesOwned ? (
    <Main {...props} />
  ) : null
}
