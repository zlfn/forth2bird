L0000:
0000:  02              PUSH_ZERO                      
0001:  02              PUSH_ZERO                      
0002:  01 0f 00 00 00  PUSH         0xf (15)          
0007:  09              JUMP_REL                           ; -> L0017
0008:  02              PUSH_ZERO                      
0009:  03 f4           PUSH_SHORT   -12               
000b:  05              LOAD_SP_REL                    
000c:  03 01           PUSH_SHORT   1                 
000e:  02              PUSH_ZERO                      
000f:  0e              SYSCALL                        
0010:  03 fc           PUSH_SHORT   -4                
0012:  06              STORE_SP_REL                   
0013:  04              POP                            
0014:  0a              JUMP_ABS                       
0015:  04              POP                            
0016:  0a              JUMP_ABS                       

L0017:
0017:  01 1b 00 00 00  PUSH         0x1b (27)         
001c:  09              JUMP_REL                           ; -> L0038

L001d:
001d:  02              PUSH_ZERO                      
001e:  03 02           PUSH_SHORT   2                 
0020:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
0025:  08              STORE_ABS                      
0026:  02              PUSH_ZERO                      
0027:  03 02           PUSH_SHORT   2                 
0029:  03 01           PUSH_SHORT   1                 
002b:  02              PUSH_ZERO                      
002c:  03 04           PUSH_SHORT   4                 
002e:  03 02           PUSH_SHORT   2                 
0030:  0e              SYSCALL                        
0031:  03 fc           PUSH_SHORT   -4                
0033:  06              STORE_SP_REL                   
0034:  04              POP                            
0035:  0a              JUMP_ABS                       
0036:  04              POP                            
0037:  0a              JUMP_ABS                       

L0038:
0038:  01 1c 00 00 00  PUSH         0x1c (28)         
003d:  09              JUMP_REL                           ; -> L005a

L003e:
003e:  02              PUSH_ZERO                      
003f:  03 03           PUSH_SHORT   3                 
0041:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
0046:  08              STORE_ABS                      
0047:  02              PUSH_ZERO                      
0048:  03 03           PUSH_SHORT   3                 
004a:  03 01           PUSH_SHORT   1                 
004c:  2c              NEG                            
004d:  02              PUSH_ZERO                      
004e:  03 04           PUSH_SHORT   4                 
0050:  03 02           PUSH_SHORT   2                 
0052:  0e              SYSCALL                        
0053:  03 fc           PUSH_SHORT   -4                
0055:  06              STORE_SP_REL                   
0056:  04              POP                            
0057:  0a              JUMP_ABS                       
0058:  04              POP                            
0059:  0a              JUMP_ABS                       

L005a:
005a:  01 1c 00 00 00  PUSH         0x1c (28)         
005f:  09              JUMP_REL                           ; -> L007c

L0060:
0060:  02              PUSH_ZERO                      
0061:  03 01           PUSH_SHORT   1                 
0063:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
0068:  08              STORE_ABS                      
0069:  02              PUSH_ZERO                      
006a:  03 01           PUSH_SHORT   1                 
006c:  02              PUSH_ZERO                      
006d:  03 01           PUSH_SHORT   1                 
006f:  2c              NEG                            
0070:  03 04           PUSH_SHORT   4                 
0072:  03 02           PUSH_SHORT   2                 
0074:  0e              SYSCALL                        
0075:  03 fc           PUSH_SHORT   -4                
0077:  06              STORE_SP_REL                   
0078:  04              POP                            
0079:  0a              JUMP_ABS                       
007a:  04              POP                            
007b:  0a              JUMP_ABS                       

L007c:
007c:  01 19 00 00 00  PUSH         0x19 (25)         
0081:  09              JUMP_REL                           ; -> L009b

L0082:
0082:  02              PUSH_ZERO                      
0083:  02              PUSH_ZERO                      
0084:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
0089:  08              STORE_ABS                      
008a:  02              PUSH_ZERO                      
008b:  02              PUSH_ZERO                      
008c:  02              PUSH_ZERO                      
008d:  03 01           PUSH_SHORT   1                 
008f:  03 04           PUSH_SHORT   4                 
0091:  03 02           PUSH_SHORT   2                 
0093:  0e              SYSCALL                        
0094:  03 fc           PUSH_SHORT   -4                
0096:  06              STORE_SP_REL                   
0097:  04              POP                            
0098:  0a              JUMP_ABS                       
0099:  04              POP                            
009a:  0a              JUMP_ABS                       

L009b:
009b:  01 17 00 00 00  PUSH         0x17 (23)         
00a0:  09              JUMP_REL                           ; -> L00b8

L00a1:
00a1:  02              PUSH_ZERO                      
00a2:  03 01           PUSH_SHORT   1                 
00a4:  01 f0 ff 00 00  PUSH         0xfff0 (65520)    
00a9:  07              LOAD_ABS                       
00aa:  02              PUSH_ZERO                      
00ab:  02              PUSH_ZERO                      
00ac:  03 04           PUSH_SHORT   4                 
00ae:  03 02           PUSH_SHORT   2                 
00b0:  0e              SYSCALL                        
00b1:  03 fc           PUSH_SHORT   -4                
00b3:  06              STORE_SP_REL                   
00b4:  04              POP                            
00b5:  0a              JUMP_ABS                       
00b6:  04              POP                            
00b7:  0a              JUMP_ABS                       

L00b8:
00b8:  01 11 00 00 00  PUSH         0x11 (17)         
00bd:  09              JUMP_REL                           ; -> L00cf

L00be:
00be:  02              PUSH_ZERO                      
00bf:  01 ff ff 00 00  PUSH         0xffff (65535)    
00c4:  03 f0           PUSH_SHORT   -16               
00c6:  05              LOAD_SP_REL                    
00c7:  25              AND                            
00c8:  03 fc           PUSH_SHORT   -4                
00ca:  06              STORE_SP_REL                   
00cb:  04              POP                            
00cc:  0a              JUMP_ABS                       
00cd:  04              POP                            
00ce:  0a              JUMP_ABS                       

L00cf:
00cf:  01 14 00 00 00  PUSH         0x14 (20)         
00d4:  09              JUMP_REL                           ; -> L00e9

L00d5:
00d5:  02              PUSH_ZERO                      
00d6:  01 ff ff 00 00  PUSH         0xffff (65535)    
00db:  03 10           PUSH_SHORT   16                
00dd:  03 ec           PUSH_SHORT   -20               
00df:  05              LOAD_SP_REL                    
00e0:  29              SHR                            
00e1:  25              AND                            
00e2:  03 fc           PUSH_SHORT   -4                
00e4:  06              STORE_SP_REL                   
00e5:  04              POP                            
00e6:  0a              JUMP_ABS                       
00e7:  04              POP                            
00e8:  0a              JUMP_ABS                       

L00e9:
00e9:  01 f3 00 00 00  PUSH         0xf3 (243)        
00ee:  01 7b 02 00 00  PUSH         0x27b (635)       
00f3:  03 01           PUSH_SHORT   1                 
00f5:  0d              SKIP                           
00f6:  0a              JUMP_ABS                       
00f7:  01 60 00 00 00  PUSH         0x60 (96)         
00fc:  0c              CALL_ABS                           ; -> L0060
00fd:  03 04           PUSH_SHORT   4                 
00ff:  05              LOAD_SP_REL                    
0100:  03 f4           PUSH_SHORT   -12               
0102:  06              STORE_SP_REL                   
0103:  01 a1 00 00 00  PUSH         0xa1 (161)        
0108:  0c              CALL_ABS                           ; -> L00a1
0109:  03 04           PUSH_SHORT   4                 
010b:  05              LOAD_SP_REL                    
010c:  04              POP                            
010d:  01 60 00 00 00  PUSH         0x60 (96)         
0112:  0c              CALL_ABS                           ; -> L0060
0113:  03 04           PUSH_SHORT   4                 
0115:  05              LOAD_SP_REL                    
0116:  03 f0           PUSH_SHORT   -16               
0118:  06              STORE_SP_REL                   
0119:  01 23 01 00 00  PUSH         0x123 (291)       
011e:  01 56 01 00 00  PUSH         0x156 (342)       
0123:  03 ec           PUSH_SHORT   -20               
0125:  05              LOAD_SP_REL                    
0126:  01 be 00 00 00  PUSH         0xbe (190)        
012b:  0c              CALL_ABS                           ; -> L00be
012c:  04              POP                            
012d:  03 08           PUSH_SHORT   8                 
012f:  05              LOAD_SP_REL                    
0130:  03 e4           PUSH_SHORT   -28               
0132:  05              LOAD_SP_REL                    
0133:  01 be 00 00 00  PUSH         0xbe (190)        
0138:  0c              CALL_ABS                           ; -> L00be
0139:  04              POP                            
013a:  03 08           PUSH_SHORT   8                 
013c:  05              LOAD_SP_REL                    
013d:  15              NE                             
013e:  0d              SKIP                           
013f:  0a              JUMP_ABS                       
0140:  03 e8           PUSH_SHORT   -24               
0142:  05              LOAD_SP_REL                    
0143:  03 ec           PUSH_SHORT   -20               
0145:  06              STORE_SP_REL                   
0146:  01 60 00 00 00  PUSH         0x60 (96)         
014b:  0c              CALL_ABS                           ; -> L0060
014c:  03 04           PUSH_SHORT   4                 
014e:  05              LOAD_SP_REL                    
014f:  03 e8           PUSH_SHORT   -24               
0151:  06              STORE_SP_REL                   
0152:  03 f8           PUSH_SHORT   -8                
0154:  05              LOAD_SP_REL                    
0155:  0a              JUMP_ABS                       
0156:  04              POP                            
0157:  01 1d 00 00 00  PUSH         0x1d (29)         
015c:  0c              CALL_ABS                           ; -> L001d
015d:  03 04           PUSH_SHORT   4                 
015f:  05              LOAD_SP_REL                    
0160:  03 f4           PUSH_SHORT   -12               
0162:  06              STORE_SP_REL                   
0163:  01 a1 00 00 00  PUSH         0xa1 (161)        
0168:  0c              CALL_ABS                           ; -> L00a1
0169:  03 04           PUSH_SHORT   4                 
016b:  05              LOAD_SP_REL                    
016c:  04              POP                            
016d:  01 1d 00 00 00  PUSH         0x1d (29)         
0172:  0c              CALL_ABS                           ; -> L001d
0173:  03 04           PUSH_SHORT   4                 
0175:  05              LOAD_SP_REL                    
0176:  03 f0           PUSH_SHORT   -16               
0178:  06              STORE_SP_REL                   
0179:  01 83 01 00 00  PUSH         0x183 (387)       
017e:  01 b6 01 00 00  PUSH         0x1b6 (438)       
0183:  03 ec           PUSH_SHORT   -20               
0185:  05              LOAD_SP_REL                    
0186:  01 d5 00 00 00  PUSH         0xd5 (213)        
018b:  0c              CALL_ABS                           ; -> L00d5
018c:  04              POP                            
018d:  03 08           PUSH_SHORT   8                 
018f:  05              LOAD_SP_REL                    
0190:  03 e4           PUSH_SHORT   -28               
0192:  05              LOAD_SP_REL                    
0193:  01 d5 00 00 00  PUSH         0xd5 (213)        
0198:  0c              CALL_ABS                           ; -> L00d5
0199:  04              POP                            
019a:  03 08           PUSH_SHORT   8                 
019c:  05              LOAD_SP_REL                    
019d:  15              NE                             
019e:  0d              SKIP                           
019f:  0a              JUMP_ABS                       
01a0:  03 e8           PUSH_SHORT   -24               
01a2:  05              LOAD_SP_REL                    
01a3:  03 ec           PUSH_SHORT   -20               
01a5:  06              STORE_SP_REL                   
01a6:  01 1d 00 00 00  PUSH         0x1d (29)         
01ab:  0c              CALL_ABS                           ; -> L001d
01ac:  03 04           PUSH_SHORT   4                 
01ae:  05              LOAD_SP_REL                    
01af:  03 e8           PUSH_SHORT   -24               
01b1:  06              STORE_SP_REL                   
01b2:  03 f8           PUSH_SHORT   -8                
01b4:  05              LOAD_SP_REL                    
01b5:  0a              JUMP_ABS                       
01b6:  04              POP                            
01b7:  01 82 00 00 00  PUSH         0x82 (130)        
01bc:  0c              CALL_ABS                           ; -> L0082
01bd:  03 04           PUSH_SHORT   4                 
01bf:  05              LOAD_SP_REL                    
01c0:  03 f4           PUSH_SHORT   -12               
01c2:  06              STORE_SP_REL                   
01c3:  01 a1 00 00 00  PUSH         0xa1 (161)        
01c8:  0c              CALL_ABS                           ; -> L00a1
01c9:  03 04           PUSH_SHORT   4                 
01cb:  05              LOAD_SP_REL                    
01cc:  04              POP                            
01cd:  01 82 00 00 00  PUSH         0x82 (130)        
01d2:  0c              CALL_ABS                           ; -> L0082
01d3:  03 04           PUSH_SHORT   4                 
01d5:  05              LOAD_SP_REL                    
01d6:  03 f0           PUSH_SHORT   -16               
01d8:  06              STORE_SP_REL                   
01d9:  01 e3 01 00 00  PUSH         0x1e3 (483)       
01de:  01 16 02 00 00  PUSH         0x216 (534)       
01e3:  03 ec           PUSH_SHORT   -20               
01e5:  05              LOAD_SP_REL                    
01e6:  01 be 00 00 00  PUSH         0xbe (190)        
01eb:  0c              CALL_ABS                           ; -> L00be
01ec:  04              POP                            
01ed:  03 08           PUSH_SHORT   8                 
01ef:  05              LOAD_SP_REL                    
01f0:  03 e4           PUSH_SHORT   -28               
01f2:  05              LOAD_SP_REL                    
01f3:  01 be 00 00 00  PUSH         0xbe (190)        
01f8:  0c              CALL_ABS                           ; -> L00be
01f9:  04              POP                            
01fa:  03 08           PUSH_SHORT   8                 
01fc:  05              LOAD_SP_REL                    
01fd:  15              NE                             
01fe:  0d              SKIP                           
01ff:  0a              JUMP_ABS                       
0200:  03 e8           PUSH_SHORT   -24               
0202:  05              LOAD_SP_REL                    
0203:  03 ec           PUSH_SHORT   -20               
0205:  06              STORE_SP_REL                   
0206:  01 82 00 00 00  PUSH         0x82 (130)        
020b:  0c              CALL_ABS                           ; -> L0082
020c:  03 04           PUSH_SHORT   4                 
020e:  05              LOAD_SP_REL                    
020f:  03 e8           PUSH_SHORT   -24               
0211:  06              STORE_SP_REL                   
0212:  03 f8           PUSH_SHORT   -8                
0214:  05              LOAD_SP_REL                    
0215:  0a              JUMP_ABS                       
0216:  04              POP                            
0217:  01 3e 00 00 00  PUSH         0x3e (62)         
021c:  0c              CALL_ABS                           ; -> L003e
021d:  03 04           PUSH_SHORT   4                 
021f:  05              LOAD_SP_REL                    
0220:  03 f4           PUSH_SHORT   -12               
0222:  06              STORE_SP_REL                   
0223:  01 a1 00 00 00  PUSH         0xa1 (161)        
0228:  0c              CALL_ABS                           ; -> L00a1
0229:  03 04           PUSH_SHORT   4                 
022b:  05              LOAD_SP_REL                    
022c:  04              POP                            
022d:  01 3e 00 00 00  PUSH         0x3e (62)         
0232:  0c              CALL_ABS                           ; -> L003e
0233:  03 04           PUSH_SHORT   4                 
0235:  05              LOAD_SP_REL                    
0236:  03 f0           PUSH_SHORT   -16               
0238:  06              STORE_SP_REL                   
0239:  01 43 02 00 00  PUSH         0x243 (579)       
023e:  01 76 02 00 00  PUSH         0x276 (630)       
0243:  03 ec           PUSH_SHORT   -20               
0245:  05              LOAD_SP_REL                    
0246:  01 d5 00 00 00  PUSH         0xd5 (213)        
024b:  0c              CALL_ABS                           ; -> L00d5
024c:  04              POP                            
024d:  03 08           PUSH_SHORT   8                 
024f:  05              LOAD_SP_REL                    
0250:  03 e4           PUSH_SHORT   -28               
0252:  05              LOAD_SP_REL                    
0253:  01 d5 00 00 00  PUSH         0xd5 (213)        
0258:  0c              CALL_ABS                           ; -> L00d5
0259:  04              POP                            
025a:  03 08           PUSH_SHORT   8                 
025c:  05              LOAD_SP_REL                    
025d:  15              NE                             
025e:  0d              SKIP                           
025f:  0a              JUMP_ABS                       
0260:  03 e8           PUSH_SHORT   -24               
0262:  05              LOAD_SP_REL                    
0263:  03 ec           PUSH_SHORT   -20               
0265:  06              STORE_SP_REL                   
0266:  01 3e 00 00 00  PUSH         0x3e (62)         
026b:  0c              CALL_ABS                           ; -> L003e
026c:  03 04           PUSH_SHORT   4                 
026e:  05              LOAD_SP_REL                    
026f:  03 e8           PUSH_SHORT   -24               
0271:  06              STORE_SP_REL                   
0272:  03 f8           PUSH_SHORT   -8                
0274:  05              LOAD_SP_REL                    
0275:  0a              JUMP_ABS                       
0276:  04              POP                            
0277:  03 f8           PUSH_SHORT   -8                
0279:  05              LOAD_SP_REL                    
027a:  0a              JUMP_ABS                       
027b:  04              POP                            
027c:  04              POP                            
027d:  04              POP                            
