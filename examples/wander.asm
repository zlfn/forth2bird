L0000:
0000:  02              PUSH_ZERO                      
0001:  01 0f 00 00 00  PUSH         0xf (15)          
0006:  09              JUMP_REL                           ; -> L0016
0007:  02              PUSH_ZERO                      
0008:  03 f4           PUSH_SHORT   -12               
000a:  05              LOAD_SP_REL                    
000b:  03 01           PUSH_SHORT   1                 
000d:  02              PUSH_ZERO                      
000e:  0e              SYSCALL                        
000f:  03 fc           PUSH_SHORT   -4                
0011:  06              STORE_SP_REL                   
0012:  04              POP                            
0013:  0a              JUMP_ABS                       
0014:  04              POP                            
0015:  0a              JUMP_ABS                       

L0016:
0016:  01 10 00 00 00  PUSH         0x10 (16)         
001b:  09              JUMP_REL                           ; -> L002c

L001c:
001c:  02              PUSH_ZERO                      
001d:  03 f4           PUSH_SHORT   -12               
001f:  05              LOAD_SP_REL                    
0020:  03 01           PUSH_SHORT   1                 
0022:  03 01           PUSH_SHORT   1                 
0024:  0e              SYSCALL                        
0025:  03 fc           PUSH_SHORT   -4                
0027:  06              STORE_SP_REL                   
0028:  04              POP                            
0029:  0a              JUMP_ABS                       
002a:  04              POP                            
002b:  0a              JUMP_ABS                       

L002c:
002c:  01 1c 00 00 00  PUSH         0x1c (28)         
0031:  09              JUMP_REL                           ; -> L004e

L0032:
0032:  02              PUSH_ZERO                      
0033:  03 01           PUSH_SHORT   1                 
0035:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
003a:  08              STORE_ABS                      
003b:  02              PUSH_ZERO                      
003c:  03 01           PUSH_SHORT   1                 
003e:  02              PUSH_ZERO                      
003f:  03 01           PUSH_SHORT   1                 
0041:  2c              NEG                            
0042:  03 04           PUSH_SHORT   4                 
0044:  03 02           PUSH_SHORT   2                 
0046:  0e              SYSCALL                        
0047:  03 fc           PUSH_SHORT   -4                
0049:  06              STORE_SP_REL                   
004a:  04              POP                            
004b:  0a              JUMP_ABS                       
004c:  04              POP                            
004d:  0a              JUMP_ABS                       

L004e:
004e:  01 19 00 00 00  PUSH         0x19 (25)         
0053:  09              JUMP_REL                           ; -> L006d

L0054:
0054:  02              PUSH_ZERO                      
0055:  02              PUSH_ZERO                      
0056:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
005b:  08              STORE_ABS                      
005c:  02              PUSH_ZERO                      
005d:  02              PUSH_ZERO                      
005e:  02              PUSH_ZERO                      
005f:  03 01           PUSH_SHORT   1                 
0061:  03 04           PUSH_SHORT   4                 
0063:  03 02           PUSH_SHORT   2                 
0065:  0e              SYSCALL                        
0066:  03 fc           PUSH_SHORT   -4                
0068:  06              STORE_SP_REL                   
0069:  04              POP                            
006a:  0a              JUMP_ABS                       
006b:  04              POP                            
006c:  0a              JUMP_ABS                       

L006d:
006d:  01 1c 00 00 00  PUSH         0x1c (28)         
0072:  09              JUMP_REL                           ; -> L008f

L0073:
0073:  02              PUSH_ZERO                      
0074:  03 03           PUSH_SHORT   3                 
0076:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
007b:  08              STORE_ABS                      
007c:  02              PUSH_ZERO                      
007d:  03 03           PUSH_SHORT   3                 
007f:  03 01           PUSH_SHORT   1                 
0081:  2c              NEG                            
0082:  02              PUSH_ZERO                      
0083:  03 04           PUSH_SHORT   4                 
0085:  03 02           PUSH_SHORT   2                 
0087:  0e              SYSCALL                        
0088:  03 fc           PUSH_SHORT   -4                
008a:  06              STORE_SP_REL                   
008b:  04              POP                            
008c:  0a              JUMP_ABS                       
008d:  04              POP                            
008e:  0a              JUMP_ABS                       

L008f:
008f:  01 1b 00 00 00  PUSH         0x1b (27)         
0094:  09              JUMP_REL                           ; -> L00b0

L0095:
0095:  02              PUSH_ZERO                      
0096:  03 02           PUSH_SHORT   2                 
0098:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
009d:  08              STORE_ABS                      
009e:  02              PUSH_ZERO                      
009f:  03 02           PUSH_SHORT   2                 
00a1:  03 01           PUSH_SHORT   1                 
00a3:  02              PUSH_ZERO                      
00a4:  03 04           PUSH_SHORT   4                 
00a6:  03 02           PUSH_SHORT   2                 
00a8:  0e              SYSCALL                        
00a9:  03 fc           PUSH_SHORT   -4                
00ab:  06              STORE_SP_REL                   
00ac:  04              POP                            
00ad:  0a              JUMP_ABS                       
00ae:  04              POP                            
00af:  0a              JUMP_ABS                       

L00b0:
00b0:  01 17 00 00 00  PUSH         0x17 (23)         
00b5:  09              JUMP_REL                           ; -> L00cd

L00b6:
00b6:  02              PUSH_ZERO                      
00b7:  03 01           PUSH_SHORT   1                 
00b9:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
00be:  07              LOAD_ABS                       
00bf:  02              PUSH_ZERO                      
00c0:  02              PUSH_ZERO                      
00c1:  03 04           PUSH_SHORT   4                 
00c3:  03 02           PUSH_SHORT   2                 
00c5:  0e              SYSCALL                        
00c6:  03 fc           PUSH_SHORT   -4                
00c8:  06              STORE_SP_REL                   
00c9:  04              POP                            
00ca:  0a              JUMP_ABS                       
00cb:  04              POP                            
00cc:  0a              JUMP_ABS                       

L00cd:
00cd:  01 00 20 00 00  PUSH         0x2000 (8192)     
00d2:  01 1c 00 00 00  PUSH         0x1c (28)         
00d7:  0c              CALL_ABS                           ; -> L001c
00d8:  04              POP                            
00d9:  03 08           PUSH_SHORT   8                 
00db:  05              LOAD_SP_REL                    
00dc:  04              POP                            
00dd:  01 2c 00 00 00  PUSH         0x2c (44)         
00e2:  09              JUMP_REL                           ; -> L010f

L00e3:
00e3:  02              PUSH_ZERO                      
00e4:  01 00 00 00 80  PUSH         0x80000000 (-2147483648)
00e9:  01 39 30 00 00  PUSH         0x3039 (12345)    
00ee:  01 00 20 00 00  PUSH         0x2000 (8192)     
00f3:  07              LOAD_ABS                       
00f4:  01 6d 4e c6 41  PUSH         0x41c64e6d (1103515245)
00f9:  22              MUL                            
00fa:  20              ADD                            
00fb:  24              REM                            
00fc:  01 00 20 00 00  PUSH         0x2000 (8192)     
0101:  08              STORE_ABS                      
0102:  01 00 20 00 00  PUSH         0x2000 (8192)     
0107:  07              LOAD_ABS                       
0108:  03 fc           PUSH_SHORT   -4                
010a:  06              STORE_SP_REL                   
010b:  04              POP                            
010c:  0a              JUMP_ABS                       
010d:  04              POP                            
010e:  0a              JUMP_ABS                       

L010f:
010f:  01 19 01 00 00  PUSH         0x119 (281)       
0114:  01 c5 01 00 00  PUSH         0x1c5 (453)       
0119:  03 01           PUSH_SHORT   1                 
011b:  0d              SKIP                           
011c:  0a              JUMP_ABS                       
011d:  03 05           PUSH_SHORT   5                 
011f:  01 e3 00 00 00  PUSH         0xe3 (227)        
0124:  0c              CALL_ABS                           ; -> L00e3
0125:  03 04           PUSH_SHORT   4                 
0127:  05              LOAD_SP_REL                    
0128:  24              REM                            
0129:  03 f4           PUSH_SHORT   -12               
012b:  06              STORE_SP_REL                   
012c:  01 11 00 00 00  PUSH         0x11 (17)         
0131:  02              PUSH_ZERO                      
0132:  03 ec           PUSH_SHORT   -20               
0134:  05              LOAD_SP_REL                    
0135:  14              EQ                             
0136:  0d              SKIP                           
0137:  09              JUMP_REL                       
0138:  04              POP                            
0139:  01 32 00 00 00  PUSH         0x32 (50)         
013e:  0c              CALL_ABS                           ; -> L0032
013f:  03 04           PUSH_SHORT   4                 
0141:  05              LOAD_SP_REL                    
0142:  04              POP                            
0143:  01 00 00 00 00  PUSH         0x0 (0)           
0148:  09              JUMP_REL                           ; -> L0149

L0149:
0149:  01 11 00 00 00  PUSH         0x11 (17)         
014e:  03 01           PUSH_SHORT   1                 
0150:  03 ec           PUSH_SHORT   -20               
0152:  05              LOAD_SP_REL                    
0153:  14              EQ                             
0154:  0d              SKIP                           
0155:  09              JUMP_REL                       
0156:  04              POP                            
0157:  01 54 00 00 00  PUSH         0x54 (84)         
015c:  0c              CALL_ABS                           ; -> L0054
015d:  03 04           PUSH_SHORT   4                 
015f:  05              LOAD_SP_REL                    
0160:  04              POP                            
0161:  01 00 00 00 00  PUSH         0x0 (0)           
0166:  09              JUMP_REL                           ; -> L0167

L0167:
0167:  01 11 00 00 00  PUSH         0x11 (17)         
016c:  03 02           PUSH_SHORT   2                 
016e:  03 ec           PUSH_SHORT   -20               
0170:  05              LOAD_SP_REL                    
0171:  14              EQ                             
0172:  0d              SKIP                           
0173:  09              JUMP_REL                       
0174:  04              POP                            
0175:  01 73 00 00 00  PUSH         0x73 (115)        
017a:  0c              CALL_ABS                           ; -> L0073
017b:  03 04           PUSH_SHORT   4                 
017d:  05              LOAD_SP_REL                    
017e:  04              POP                            
017f:  01 00 00 00 00  PUSH         0x0 (0)           
0184:  09              JUMP_REL                           ; -> L0185

L0185:
0185:  01 11 00 00 00  PUSH         0x11 (17)         
018a:  03 03           PUSH_SHORT   3                 
018c:  03 ec           PUSH_SHORT   -20               
018e:  05              LOAD_SP_REL                    
018f:  14              EQ                             
0190:  0d              SKIP                           
0191:  09              JUMP_REL                       
0192:  04              POP                            
0193:  01 95 00 00 00  PUSH         0x95 (149)        
0198:  0c              CALL_ABS                           ; -> L0095
0199:  03 04           PUSH_SHORT   4                 
019b:  05              LOAD_SP_REL                    
019c:  04              POP                            
019d:  01 00 00 00 00  PUSH         0x0 (0)           
01a2:  09              JUMP_REL                           ; -> L01a3

L01a3:
01a3:  01 11 00 00 00  PUSH         0x11 (17)         
01a8:  03 04           PUSH_SHORT   4                 
01aa:  03 ec           PUSH_SHORT   -20               
01ac:  05              LOAD_SP_REL                    
01ad:  14              EQ                             
01ae:  0d              SKIP                           
01af:  09              JUMP_REL                       
01b0:  04              POP                            
01b1:  01 b6 00 00 00  PUSH         0xb6 (182)        
01b6:  0c              CALL_ABS                           ; -> L00b6
01b7:  03 04           PUSH_SHORT   4                 
01b9:  05              LOAD_SP_REL                    
01ba:  04              POP                            
01bb:  01 00 00 00 00  PUSH         0x0 (0)           
01c0:  09              JUMP_REL                           ; -> L01c1

L01c1:
01c1:  03 f8           PUSH_SHORT   -8                
01c3:  05              LOAD_SP_REL                    
01c4:  0a              JUMP_ABS                       
01c5:  04              POP                            
01c6:  04              POP                            
